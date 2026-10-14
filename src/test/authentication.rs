use std::collections::HashSet;

use anyhow::{Result, anyhow};
#[cfg(test)]
use chromiumoxide::Browser;
use chromiumoxide::Page;
use serde_json::{Value, json};

use crate::{config::basic_auth, types::BasicAuth};

use super::cdp::Client;

pub(super) struct Authentication {
    task: tokio::task::JoinHandle<Result<()>>,
}

impl Authentication {
    #[cfg(test)]
    pub async fn install(
        browser: &Browser,
        page: &Page,
        entries: &[BasicAuth],
    ) -> Result<Option<Self>> {
        if entries.is_empty() {
            return Ok(None);
        }
        let client = Client::connect(browser).await?;
        Self::install_with_client(client, page, entries).await
    }

    pub(super) async fn install_with_client(
        client: Client,
        page: &Page,
        entries: &[BasicAuth],
    ) -> Result<Option<Self>> {
        if entries.is_empty() {
            return Ok(None);
        }
        let session = client.attach(page.target_id().as_ref()).await?;
        let authentication = Self::prepare_session(client.clone(), &session, entries)?;
        client
            .call(
                Some(&session),
                "Fetch.enable",
                json!({"handleAuthRequests": true}),
            )
            .await?;
        Ok(authentication)
    }

    pub(super) fn prepare_session(
        client: Client,
        session: &str,
        entries: &[BasicAuth],
    ) -> Result<Option<Self>> {
        if entries.is_empty() {
            return Ok(None);
        }
        let entries = basic_auth::resolve(entries)?;
        let session = session.to_string();
        let mut events = client.subscribe();
        let task = tokio::spawn(async move {
            let mut attempted = HashSet::new();
            loop {
                let event = events
                    .recv()
                    .await
                    .map_err(|_| anyhow!("Basic Auth event stream closed or overflowed"))?;
                if event.session_id.as_deref() != Some(session.as_str()) {
                    continue;
                }
                match event.method.as_str() {
                    "Fetch.requestPaused" => {
                        if let Err(error) = client
                            .call(
                                Some(&session),
                                "Fetch.continueRequest",
                                json!({"requestId": event.params["requestId"]}),
                            )
                            .await
                            && !is_canceled_request_error(&error)
                        {
                            return Err(error);
                        }
                    }
                    "Fetch.authRequired" => {
                        let request_id = event.params["requestId"]
                            .as_str()
                            .ok_or_else(|| anyhow!("Missing auth request ID"))?;
                        let challenge = &event.params["authChallenge"];
                        let origin = challenge["origin"]
                            .as_str()
                            .and_then(|origin| reqwest::Url::parse(origin).ok())
                            .map(|url| url.origin().ascii_serialization());
                        let entry = entries
                            .iter()
                            .find(|entry| Some(entry.origin.as_str()) == origin.as_deref());
                        let response = auth_response(
                            entry,
                            challenge,
                            attempted.insert(request_id.to_string()),
                        );
                        if let Err(error) = client
                            .call(
                                Some(&session),
                                "Fetch.continueWithAuth",
                                json!({"requestId": request_id, "authChallengeResponse": response}),
                            )
                            .await
                            && !is_canceled_request_error(&error)
                        {
                            return Err(error);
                        }
                    }
                    _ => {}
                }
            }
        });
        Ok(Some(Self { task }))
    }

    pub fn check(&self) -> Result<()> {
        if self.task.is_finished() {
            return Err(anyhow!(
                "Basic Auth handler stopped before the scenario completed"
            ));
        }
        Ok(())
    }
}

impl Drop for Authentication {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn is_canceled_request_error(error: &anyhow::Error) -> bool {
    // A subresource request can be aborted by the page (e.g. AbortController or navigation)
    // while Fetch.requestPaused is in flight. Chromium rejects the stale requestId with a
    // protocol error, which must not stop the Fetch listener for subsequent requests.
    error.to_string().starts_with("CDP command failed:")
}

fn auth_response(entry: Option<&BasicAuth>, challenge: &Value, first_attempt: bool) -> Value {
    if first_attempt
        && challenge["source"] == "Server"
        && challenge["scheme"]
            .as_str()
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("basic"))
        && let Some(entry) = entry
    {
        json!({"response": "ProvideCredentials", "username": entry.username, "password": entry.password})
    } else {
        json!({"response": "CancelAuth"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_never_sent_to_proxies_unknown_origins_or_repeated_challenges() {
        let entry = BasicAuth {
            origin: "https://example.com".into(),
            username: "user".into(),
            password: "secret".into(),
        };
        let server = json!({"source": "Server", "scheme": "basic"});
        assert_eq!(
            auth_response(Some(&entry), &server, true)["response"],
            "ProvideCredentials"
        );
        for response in [
            auth_response(None, &server, true),
            auth_response(Some(&entry), &server, false),
            auth_response(
                Some(&entry),
                &json!({"source": "Proxy", "scheme": "basic"}),
                true,
            ),
            auth_response(
                Some(&entry),
                &json!({"source": "Server", "scheme": "digest"}),
                true,
            ),
        ] {
            assert_eq!(response, json!({"response": "CancelAuth"}));
        }
    }

    #[test]
    fn canceled_request_protocol_errors_do_not_abort_authentication_loop() {
        assert!(is_canceled_request_error(&anyhow!(
            "CDP command failed: Invalid InterceptionId."
        )));
        assert!(!is_canceled_request_error(&anyhow!(
            "CDP connection closed during Fetch.continueRequest"
        )));
        assert!(!is_canceled_request_error(&anyhow!(
            "Timed out waiting for CDP Fetch.continueRequest"
        )));
    }
}
