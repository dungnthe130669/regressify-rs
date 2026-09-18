use std::collections::HashSet;

use anyhow::{Result, anyhow};
use chromiumoxide::{Browser, Page};
use serde_json::{Value, json};

use crate::{config::basic_auth, types::BasicAuth};

use super::cdp::Client;

pub(super) struct Authentication {
    task: tokio::task::JoinHandle<Result<()>>,
}

impl Authentication {
    pub async fn install(
        browser: &Browser,
        page: &Page,
        entries: &[BasicAuth],
    ) -> Result<Option<Self>> {
        if entries.is_empty() {
            return Ok(None);
        }
        let entries = basic_auth::resolve(entries)?;
        let client = Client::connect(browser).await?;
        let session = client.attach(page.target_id().as_ref()).await?;
        let mut events = client.subscribe();
        client
            .call(
                Some(&session),
                "Fetch.enable",
                json!({"handleAuthRequests": true}),
            )
            .await?;
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
                        client
                            .call(
                                Some(&session),
                                "Fetch.continueRequest",
                                json!({"requestId": event.params["requestId"]}),
                            )
                            .await?;
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
                        client
                            .call(
                                Some(&session),
                                "Fetch.continueWithAuth",
                                json!({"requestId": request_id, "authChallengeResponse": response}),
                            )
                            .await?;
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
}
