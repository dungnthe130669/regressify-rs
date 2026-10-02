use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tracing::{Instrument, Subscriber, span::Attributes};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

use super::{
    adapter::{DiagnosticContext, event_record, store_context},
    health::{HealthSnapshot, JournalOwner, JournalSink},
    install,
    journal::{ArchiveState, OperationMetadata},
    record::*,
};

static ROUTES: OnceLock<Mutex<HashMap<OperationId, Weak<JournalSink>>>> = OnceLock::new();

tokio::task_local! {
    static CURRENT: Arc<OperationContext>;
    static ACTION: Vec<(Field, FieldValue)>;
    static STAGE_SPAN: (SpanId, Option<SpanId>);
}

pub struct JournalLayer;

impl<Sub> Layer<Sub> for JournalLayer
where
    Sub: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attributes: &Attributes<'_>,
        id: &tracing::Id,
        context: Context<'_, Sub>,
    ) {
        store_context(attributes, id, context);
    }

    fn on_event(&self, event: &tracing::Event<'_>, context: Context<'_, Sub>) {
        let Some(record) = event_record(event, context) else {
            return;
        };
        let sink = ROUTES
            .get()
            .and_then(|routes| routes.lock().ok()?.get(&record.operation_id)?.upgrade());
        if let Some(sink) = sink {
            sink.record(&record);
        }
    }
}

#[derive(Default)]
pub struct Completion {
    pub outcome: Option<Outcome>,
    pub error: Option<SanitizedError>,
    pub duration_ms: Option<u64>,
}

pub struct OperationContext {
    pub identity: DiagnosticContext,
    sink: Arc<JournalSink>,
    started: Instant,
    sequence: AtomicU64,
    action_index: AtomicU64,
    display_level: std::sync::atomic::AtomicU8,
}

impl OperationContext {
    pub fn set_display_level(&self, level: Severity) {
        self.display_level.store(level as u8, Ordering::Relaxed);
    }

    pub fn display_level(&self) -> Option<Severity> {
        match self.display_level.load(Ordering::Relaxed) {
            0 => Some(Severity::Trace),
            1 => Some(Severity::Debug),
            2 => Some(Severity::Info),
            3 => Some(Severity::Warn),
            4 => Some(Severity::Error),
            _ => None,
        }
    }

    pub fn record(
        &self,
        stage: Stage,
        event: DiagnosticEvent,
        severity: Severity,
        fields: impl IntoIterator<Item = (Field, FieldValue)>,
        completion: Completion,
    ) {
        let mut record = DiagnosticRecordV1::new(
            self.identity.operation_id.clone(),
            self.identity.operation_kind,
            stage,
            event,
        );
        record.capture_run_id = self.identity.capture_run_id.clone();
        record.execution = self.identity.execution.clone();
        record.sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        record.elapsed_ms = self.started.elapsed().as_millis() as u64;
        record.severity = severity;
        if stage == Stage::ActionsFrames {
            let _ = ACTION.try_with(|fields| record.fields.extend(fields.iter().cloned()));
        }
        record.fields.extend(fields);
        record.outcome = completion.outcome;
        record.error_cause = completion.error;
        record.duration_ms = completion.duration_ms;
        let _ = STAGE_SPAN.try_with(|(span, parent)| {
            record.span_id = span.clone();
            record.parent_span = parent.clone();
        });
        if install::current().is_some_and(|installation| installation.installed()) {
            let _ = super::adapter::emit(&record);
        } else {
            self.sink.record(&record);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveReference {
    pub operation_id: OperationId,
    pub execution_id: Option<ExecutionId>,
    pub state: ArchiveState,
    pub missing_records: Option<u64>,
}

pub struct Operation {
    context: Arc<OperationContext>,
    owner: Option<JournalOwner>,
}

impl Operation {
    pub fn start(
        output: &Path,
        kind: OperationKind,
        run: Option<CaptureRunId>,
        execution: Option<ExecutionIdentity>,
        job: Option<usize>,
    ) -> Self {
        Self::start_identified(output, kind, run, execution, job, None)
    }

    pub fn start_identified(
        output: &Path,
        kind: OperationKind,
        run: Option<CaptureRunId>,
        execution: Option<ExecutionIdentity>,
        job: Option<usize>,
        operation_id: Option<OperationId>,
    ) -> Self {
        let mut metadata = OperationMetadata::new(kind, run);
        if let Some(operation_id) = operation_id {
            metadata.operation_id = operation_id;
        }
        let warning_job = job;
        let owner = JournalOwner::start(
            output,
            metadata.clone(),
            crate::common::DIAGNOSTIC_OPTIONS
                .get()
                .copied()
                .unwrap_or_else(|| {
                    super::journal::JournalOptions::resolve(None, None, |name| {
                        std::env::var(name).ok()
                    })
                    .unwrap_or_default()
                }),
            crate::common::DISABLE_DIAGNOSTIC_ARCHIVE
                .get()
                .copied()
                .unwrap_or_else(|| {
                    super::health::archives_disabled(|name| std::env::var(name).ok())
                        .unwrap_or(true)
                }),
            move |health| {
                if let Some(warning) = health.warning {
                    install::emergency_warning(warning.message());
                    if let Some(id) = warning_job {
                        crate::progress::emit(crate::progress::Event::Log {
                            id,
                            message: warning.message().into(),
                        });
                    }
                }
            },
        );
        let sink = owner.sink();
        let context = Arc::new(OperationContext {
            identity: DiagnosticContext {
                operation_id: metadata.operation_id.clone(),
                operation_kind: kind,
                capture_run_id: metadata.capture_run_id,
                execution,
            },
            sink: sink.clone(),
            started: Instant::now(),
            sequence: AtomicU64::new(0),
            action_index: AtomicU64::new(0),
            display_level: std::sync::atomic::AtomicU8::new(u8::MAX),
        });
        if let Ok(mut routes) = ROUTES.get_or_init(Default::default).lock() {
            routes.insert(metadata.operation_id, Arc::downgrade(&sink));
        }
        if let (Some(job), Some(execution), Some(installation)) =
            (job, context.identity.execution.as_ref(), install::current())
        {
            let _ = installation
                .adapter
                .bind(execution.execution_id.clone(), job);
        }
        Self {
            context,
            owner: Some(owner),
        }
    }

    pub fn context(&self) -> Arc<OperationContext> {
        self.context.clone()
    }

    pub fn reference(&self) -> ArchiveReference {
        let health = self.context.sink.health();
        ArchiveReference {
            operation_id: self.context.identity.operation_id.clone(),
            execution_id: self
                .context
                .identity
                .execution
                .as_ref()
                .map(|execution| execution.execution_id.clone()),
            state: health.state,
            missing_records: health.missing_records,
        }
    }

    fn record_finish(&self, outcome: Outcome) {
        in_scope(Some(self.context.clone()), || {
            self.context.record(
                Stage::Finish,
                DiagnosticEvent::Finished,
                outcome.severity(),
                [],
                Completion {
                    outcome: Some(outcome),
                    error: None,
                    duration_ms: Some(self.context.started.elapsed().as_millis() as u64),
                },
            )
        });
    }

    pub fn finish_blocking(mut self, outcome: Outcome) -> ArchiveReference {
        self.record_finish(outcome);
        let owner = self
            .owner
            .take()
            .expect("operation owns its journal until finalization");
        let health = owner.finish(outcome);
        self.finalized_reference(health)
    }

    pub async fn finish(mut self, outcome: Outcome) -> ArchiveReference {
        self.record_finish(outcome);
        let owner = self
            .owner
            .take()
            .expect("operation owns its journal until finalization");
        let health = tokio::task::spawn_blocking(move || owner.finish(outcome))
            .await
            .unwrap_or(HealthSnapshot {
                state: ArchiveState::Partial,
                warning: None,
                records_written: 0,
                missing_records: None,
            });
        self.finalized_reference(health)
    }

    fn finalized_reference(&self, health: HealthSnapshot) -> ArchiveReference {
        ArchiveReference {
            operation_id: self.context.identity.operation_id.clone(),
            execution_id: self
                .context
                .identity
                .execution
                .as_ref()
                .map(|execution| execution.execution_id.clone()),
            state: health.state,
            missing_records: health.missing_records,
        }
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        if self.owner.is_some() {
            self.context.record(
                Stage::CompletionCleanup,
                DiagnosticEvent::Cancelled,
                Severity::Warn,
                [(Field::State, FieldValue::Kind(SafeValue::Unknown))],
                Completion {
                    outcome: Some(
                        if self.context.identity.operation_kind == OperationKind::Deployment {
                            Outcome::CancelledRemoteUnknown
                        } else {
                            Outcome::Cancelled
                        },
                    ),
                    error: None,
                    duration_ms: None,
                },
            );
        }
        if let Some(routes) = ROUTES.get()
            && let Ok(mut routes) = routes.lock()
        {
            routes.remove(&self.context.identity.operation_id);
        }
        if let (Some(execution), Some(installation)) =
            (self.context.identity.execution.as_ref(), install::current())
        {
            installation.adapter.unbind(&execution.execution_id);
        }
    }
}

pub fn current() -> Option<Arc<OperationContext>> {
    CURRENT.try_with(Arc::clone).ok()
}

pub fn next_action_index() -> u64 {
    current()
        .map(|context| context.action_index.fetch_add(1, Ordering::Relaxed) + 1)
        .unwrap_or(1)
}

pub async fn action_scope<Work: Future>(
    fields: Vec<(Field, FieldValue)>,
    work: Work,
) -> Work::Output {
    ACTION.scope(fields, work).await
}

pub fn in_scope<Output>(
    context: Option<Arc<OperationContext>>,
    work: impl FnOnce() -> Output,
) -> Output {
    match context {
        Some(context) => {
            let span = context.identity.span();
            CURRENT.sync_scope(context, || span.in_scope(work))
        }
        None => work(),
    }
}

pub async fn scope<Work: Future>(
    context: Option<Arc<OperationContext>>,
    work: Work,
) -> Work::Output {
    match context {
        Some(context) => {
            let span = context.identity.span();
            CURRENT.scope(context, work).instrument(span).await
        }
        None => work.await,
    }
}

pub fn event(
    stage: Stage,
    event: DiagnosticEvent,
    severity: Severity,
    fields: impl IntoIterator<Item = (Field, FieldValue)>,
) {
    if let Some(context) = current() {
        context.record(stage, event, severity, fields, Completion::default());
    }
}

pub fn result(stage: Stage, result: &anyhow::Result<()>, duration_ms: u64) {
    if let Some(context) = current() {
        context.record(
            stage,
            DiagnosticEvent::StageFinished,
            if result.is_ok() {
                Severity::Info
            } else {
                Severity::Error
            },
            [],
            Completion {
                outcome: Some(if result.is_ok() {
                    Outcome::Success
                } else {
                    Outcome::Failure
                }),
                error: result.as_ref().err().map(SanitizedError::capture),
                duration_ms: Some(duration_ms),
            },
        );
    }
}

pub fn stage_sync<Output>(
    stage: Stage,
    fields: impl IntoIterator<Item = (Field, FieldValue)>,
    work: impl FnOnce() -> anyhow::Result<Output>,
) -> anyhow::Result<Output> {
    let span = (
        SpanId::new(),
        STAGE_SPAN.try_with(|(span, _)| span.clone()).ok(),
    );
    STAGE_SPAN.sync_scope(span, || {
        let fields: Vec<_> = fields.into_iter().collect();
        event(
            stage,
            DiagnosticEvent::StageStarted,
            Severity::Info,
            fields.iter().cloned(),
        );
        let started = Instant::now();
        let result = work();
        if let Some(context) = current() {
            context.record(
                stage,
                DiagnosticEvent::StageFinished,
                if result.is_ok() {
                    Severity::Info
                } else {
                    Severity::Error
                },
                fields,
                Completion {
                    outcome: Some(if result.is_ok() {
                        Outcome::Success
                    } else {
                        Outcome::Failure
                    }),
                    error: result.as_ref().err().map(SanitizedError::capture),
                    duration_ms: Some(started.elapsed().as_millis() as u64),
                },
            );
        }
        result
    })
}

pub async fn navigation<Output>(
    destination: &str,
    timeout_ms: u64,
    work: impl Future<Output = anyhow::Result<Output>>,
) -> anyhow::Result<Output> {
    stage(
        Stage::Navigation,
        [
            (
                Field::Destination,
                FieldValue::Identity(SafeIdentity::fingerprint(destination)),
            ),
            (Field::TimeoutMs, FieldValue::Number(timeout_ms)),
        ],
        work,
    )
    .await
}

pub fn stage<Output>(
    stage: Stage,
    fields: impl IntoIterator<Item = (Field, FieldValue)>,
    work: impl Future<Output = anyhow::Result<Output>>,
) -> impl Future<Output = anyhow::Result<Output>> {
    let fields: Vec<_> = fields.into_iter().collect();
    let work = Box::pin(work);
    async move {
        let span = (
            SpanId::new(),
            STAGE_SPAN.try_with(|(span, _)| span.clone()).ok(),
        );
        STAGE_SPAN
            .scope(span, async move {
                event(
                    stage,
                    DiagnosticEvent::StageStarted,
                    Severity::Info,
                    fields.iter().cloned(),
                );
                let started = Instant::now();
                let result = work.await;
                if let Some(context) = current() {
                    context.record(
                        stage,
                        DiagnosticEvent::StageFinished,
                        if result.is_ok() {
                            Severity::Info
                        } else {
                            Severity::Error
                        },
                        fields,
                        Completion {
                            outcome: Some(if result.is_ok() {
                                Outcome::Success
                            } else {
                                Outcome::Failure
                            }),
                            error: result.as_ref().err().map(SanitizedError::capture),
                            duration_ms: Some(started.elapsed().as_millis() as u64),
                        },
                    );
                }
                result
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diagnostics::{
            adapter::{DiagnosticLayer, LiveProgressAdapter},
            journal::{ArchiveState, JournalReader},
        },
        progress,
    };
    use tracing::instrument::WithSubscriber;
    use tracing_subscriber::prelude::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn interleaved_executions_survive_live_eviction_and_propagate_blocking_context() {
        install::install_subscriber();
        let root = tempfile::tempdir().unwrap();
        let live = Arc::new(Mutex::new(progress::LiveProgress::default()));
        let received = live.clone();
        let adapter = Arc::new(LiveProgressAdapter::new(move |event| {
            received.lock().unwrap().apply(progress::Record {
                version: 1,
                elapsed_ms: 0,
                event,
            })
        }));
        let subscriber = tracing_subscriber::registry()
            .with(JournalLayer)
            .with(DiagnosticLayer::new(adapter.clone(), |_| {}));
        let dispatch = tracing::Dispatch::new(subscriber);
        let mut tasks = Vec::new();
        for job in 1..=2 {
            live.lock().unwrap().apply(progress::Record {
                version: 1,
                elapsed_ms: 0,
                event: progress::Event::Queued {
                    job: progress::Job {
                        id: job,
                        suite_id: format!("suite-{job}"),
                        suite_name: "Suite".into(),
                        scenario_id: "page".into(),
                        label: "Page".into(),
                        viewport: format!("viewport-{job}"),
                        captures: vec![],
                    },
                },
            });
            let execution = ExecutionIdentity {
                execution_id: ExecutionId::new(),
                suite_id: SafeIdentity::fingerprint(format!("suite-{job}")),
                scenario_id: SafeIdentity::fingerprint("page"),
                viewport: ViewportIdentity {
                    name: SafeIdentity::fingerprint(format!("viewport-{job}")),
                    width: 320 * job as u32,
                    height: 480,
                },
                capture_role: if job == 1 {
                    CaptureRole::Reference
                } else {
                    CaptureRole::Test
                },
                attempt: job as u32,
            };
            adapter.bind(execution.execution_id.clone(), job).unwrap();
            let operation = Operation::start(
                root.path(),
                OperationKind::Capture,
                None,
                Some(execution.clone()),
                None,
            );
            let context = operation.context();
            let dispatch = dispatch.clone();
            tasks.push(tokio::spawn(
                async move {
                    scope(Some(context.clone()), async {
                        for index in 0..4200 {
                            event(
                                Stage::ActionsFrames,
                                DiagnosticEvent::Decision,
                                Severity::Debug,
                                [(Field::ActionIndex, FieldValue::Number(index))],
                            );
                            if (index + 1) % 16 == 0 {
                                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                                    while context.sink.health().records_written < index + 1 {
                                        tokio::task::yield_now().await;
                                    }
                                })
                                .await
                                .unwrap();
                            }
                        }
                        let blocking_context = current();
                        tokio::task::spawn_blocking(move || {
                            in_scope(blocking_context, || {
                                event(
                                    Stage::CaptureArtifacts,
                                    DiagnosticEvent::Captured,
                                    Severity::Info,
                                    [(Field::Bytes, FieldValue::Number(42))],
                                );
                            })
                        })
                        .await
                        .unwrap();
                        stage(
                            Stage::StateHooks,
                            [],
                            stage(Stage::ActionsFrames, [], async { Ok(()) }),
                        )
                        .await
                        .unwrap();
                    })
                    .await;
                    let reference = operation
                        .finish(if job == 1 {
                            Outcome::Success
                        } else {
                            Outcome::Failure
                        })
                        .await;
                    (reference, execution)
                }
                .with_subscriber(dispatch),
            ));
        }
        for task in tasks {
            let (reference, execution) = task.await.unwrap();
            assert_eq!(reference.state, ArchiveState::Complete);
            let reader = JournalReader::open(root.path(), &reference.operation_id).unwrap();
            let mut count = 0;
            let mut spans = Vec::new();
            reader
                .visit(|record| {
                    assert_eq!(record.sequence, count);
                    count += 1;
                    if let Some(identity) = &record.execution {
                        assert_eq!(identity, &execution);
                    }
                    if record.event_name == DiagnosticEvent::StageStarted {
                        spans.push((record.span_id, record.parent_span));
                    }
                })
                .unwrap();
            assert_eq!(count, 4207);
            assert_eq!(spans[1].1, Some(spans[0].0.clone()));
        }
        assert!(live.lock().unwrap().omitted_logs > 0);
        assert!(live.lock().unwrap().logs.len() <= 4096);
        live.lock().unwrap().clear_logs();
    }
}
