use std::{
    future::Future,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow, bail, ensure};
use chromiumoxide::cdp::browser_protocol::page::HandleJavaScriptDialogParams;

use crate::types::{DialogAction, DialogKind, DialogResponse, TestAction};

use super::cdp::Client;

#[derive(Default)]
pub(super) struct DialogState {
    pending: Mutex<Option<DialogAction>>,
    cancelled: AtomicBool,
}

impl DialogState {
    pub fn arm(&self, action: DialogAction) -> Result<()> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| anyhow!("Dialog state lock poisoned"))?;
        ensure!(
            pending.is_none(),
            "The previous dialog expectation has no triggering action"
        );
        *pending = Some(action);
        Ok(())
    }

    pub fn ensure_clear(&self) -> Result<()> {
        ensure!(
            self.pending
                .lock()
                .map_err(|_| anyhow!("Dialog state lock poisoned"))?
                .is_none(),
            "dialog has no following applicable triggering action in this tab"
        );
        Ok(())
    }

    pub fn navigation_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub async fn run(
        &self,
        client: &Client,
        session: &str,
        action: &TestAction,
        timeout: Duration,
        work: impl Future<Output = Result<()>>,
    ) -> Result<()> {
        self.cancelled.store(false, Ordering::Release);
        let expectation = self
            .pending
            .lock()
            .map_err(|_| anyhow!("Dialog state lock poisoned"))?
            .take();
        let Some(expectation) = expectation else {
            return work.await;
        };
        ensure!(
            action.can_trigger_dialog(),
            "dialog must be followed by an applicable triggering action"
        );
        let mut events = client.subscribe();
        let deadline = tokio::time::Instant::now() + timeout;
        let mut handled = false;
        let mut finished = false;
        tokio::pin!(work);
        crate::diagnostics::runtime::stage(
            crate::diagnostics::record::Stage::ActionsFrames,
            [(crate::diagnostics::record::Field::Condition, crate::diagnostics::record::FieldValue::Kind(crate::diagnostics::record::SafeValue::Dialog))],
            async {
                loop {
                    tokio::select! {
                        result = &mut work, if !finished => {
                            result?;
                            finished = true;
                        }
                        event = events.recv() => {
                            let event = event.map_err(|_| anyhow!("Dialog event stream closed or overflowed"))?;
                            if event.session_id.as_deref() != Some(session) || event.method != "Page.javascriptDialogOpening" {
                                continue;
                            }
                            let kind = serde_json::from_value::<DialogKind>(event.params["type"].clone());
                            let matches = !handled && kind.as_ref().is_ok_and(|kind| expectation.kind.is_none_or(|expected| expected == *kind))
                                && expectation.message.as_ref().is_none_or(|message| event.params["message"].as_str() == Some(message.as_str()));
                            let accept = matches && expectation.dialog == DialogResponse::Accept;
                            // Dismissed beforeunload is an intentional cancelled navigation, not a network failure.
                            if matches && kind.as_ref().is_ok_and(|kind| *kind == DialogKind::BeforeUnload) && !accept {
                                self.cancelled.store(true, Ordering::Release);
                            }
                            let mut command = HandleJavaScriptDialogParams::new(accept);
                            if matches {
                                command.prompt_text = expectation.prompt_text.clone();
                            }
                            client.call_with_timeout(Some(session), HandleJavaScriptDialogParams::IDENTIFIER, serde_json::to_value(command)?, timeout).await?;
                            ensure!(matches, "Unexpected, extra, or mismatched JavaScript dialog");
                            handled = true;
                        }
                        _ = tokio::time::sleep_until(deadline), if !handled => {
                            bail!("Expected JavaScript dialog did not appear within {}ms", timeout.as_millis());
                        }
                    }
                    if finished && handled {
                        return Ok(());
                    }
                }
            },
        ).await
    }
}
