use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Result, anyhow};
use chromiumoxide::{Browser, Page};
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::types::Needs;

use super::{capture::load_persisted_state, cdp::Client};

type Origins = BTreeMap<String, BTreeMap<String, String>>;

pub(super) fn validate_origins(value: &Value) -> Result<Origins> {
    let Some(origins) = value.get("origins") else {
        return Ok(Origins::new());
    };
    let origins = origins
        .as_array()
        .ok_or_else(|| anyhow!("Persisted state origins must be an array"))?;
    let mut result = Origins::new();
    for entry in origins {
        let origin = entry
            .get("origin")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Persisted state origin must be a string"))?;
        let url =
            reqwest::Url::parse(origin).map_err(|_| anyhow!("Invalid persisted storage origin"))?;
        anyhow::ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.origin().ascii_serialization() == origin,
            "Persisted storage origin must be an exact HTTP(S) origin"
        );
        let items = entry
            .get("localStorage")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Persisted origin localStorage must be an array"))?;
        let storage = result.entry(origin.to_string()).or_default();
        for item in items {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("Persisted localStorage name must be a string"))?;
            let value = item
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("Persisted localStorage value must be a string"))?;
            storage.insert(name.to_string(), value.to_string());
        }
    }
    Ok(result)
}

fn restoration_script(origins: &Origins) -> Result<String> {
    Ok(format!(
        r#"(() => {{
        const origins = {};
        const values = origins[location.origin];
        if (!values) return;
        const marker = '__regressify_restored_' + location.origin;
        if (sessionStorage.getItem(marker)) return;
        for (const [name, value] of Object.entries(values)) localStorage.setItem(name, value);
        sessionStorage.setItem(marker, '1');
    }})()"#,
        serde_json::to_string(origins)?
    ))
}

pub(super) struct StorageState {
    snapshots: mpsc::Sender<oneshot::Sender<Origins>>,
    error: Arc<Mutex<Option<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl StorageState {
    pub async fn install(
        browser: &Browser,
        page: &Page,
        needs: Option<&Needs>,
        initial_url: &str,
    ) -> Result<Self> {
        let mut origins = Origins::new();
        let names = match needs {
            Some(Needs::String(name)) => vec![name.as_str()],
            Some(Needs::StringArray(names)) => names.iter().map(String::as_str).collect(),
            None => Vec::new(),
        };
        for name in names {
            let (_, value) = load_persisted_state(name)?;
            for (origin, items) in validate_origins(&value)? {
                origins.entry(origin).or_default().extend(items);
            }
            if let Some(legacy) = value.get("localStorage").and_then(Value::as_object) {
                let origin = reqwest::Url::parse(initial_url)?
                    .origin()
                    .ascii_serialization();
                for (name, value) in legacy {
                    origins.entry(origin.clone()).or_default().insert(
                        name.clone(),
                        value
                            .as_str()
                            .ok_or_else(|| anyhow!("Legacy localStorage value must be a string"))?
                            .to_string(),
                    );
                }
            }
        }
        Self::install_origins(browser, page, origins).await
    }

    async fn install_origins(browser: &Browser, page: &Page, mut origins: Origins) -> Result<Self> {
        page.evaluate_on_new_document(restoration_script(&origins)?)
            .await?;
        let client = Client::connect(browser).await?;
        let session = client.attach(page.target_id().as_ref()).await?;
        let mut events = client.subscribe();
        client
            .call(Some(&session), "DOMStorage.enable", json!({}))
            .await?;
        let (snapshots, mut requests) = mpsc::channel::<oneshot::Sender<Origins>>(4);
        let error = Arc::new(Mutex::new(None));
        let error_sink = error.clone();
        let task = tokio::spawn(async move {
            let _connection = client;
            loop {
                tokio::select! {
                    biased;
                    event = events.recv() => match event {
                        Ok(event) => {
                            if event.session_id.as_deref() == Some(session.as_str()) {
                                update_storage(&mut origins, &event.method, &event.params);
                            }
                        }
                        Err(_) => { *error_sink.lock().await = Some("Persisted-state event stream closed or overflowed".into()); break; }
                    },
                    request = requests.recv() => match request {
                        Some(reply) => { let _ = reply.send(origins.clone()); }
                        None => break,
                    }
                }
            }
        });
        Ok(Self {
            snapshots,
            error,
            task,
        })
    }

    pub async fn capture(&self, page: &Page) -> Result<Value> {
        let current: Value = page
            .evaluate(
                r#"(() => ({origin: location.origin, items: Object.entries(localStorage)}))()"#,
            )
            .await?
            .into_value()?;
        if let Some(error) = self.error.lock().await.as_ref() {
            return Err(anyhow!("{error}"));
        }
        let (reply, response) = oneshot::channel();
        self.snapshots
            .send(reply)
            .await
            .map_err(|_| anyhow!("State tracker stopped"))?;
        let mut origins = response
            .await
            .map_err(|_| anyhow!("State tracker stopped"))?;
        if let (Some(origin), Some(items)) =
            (current["origin"].as_str(), current["items"].as_array())
        {
            let storage = origins.entry(origin.to_string()).or_default();
            storage.clear();
            for item in items {
                if let (Some(name), Some(value)) = (item[0].as_str(), item[1].as_str()) {
                    storage.insert(name.into(), value.into());
                }
            }
        }
        let origins = origins.into_iter().map(|(origin, items)| json!({
            "origin": origin,
            "localStorage": items.into_iter().map(|(name, value)| json!({"name": name, "value": value})).collect::<Vec<_>>()
        })).collect::<Vec<_>>();
        let cookies = page
            .execute(chromiumoxide::cdp::browser_protocol::storage::GetCookiesParams::default())
            .await?
            .result
            .cookies;
        Ok(json!({"cookies": cookies, "origins": origins}))
    }
}

impl Drop for StorageState {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn update_storage(origins: &mut Origins, method: &str, params: &Value) {
    let storage = &params["storageId"];
    if storage["isLocalStorage"] != true {
        return;
    }
    let Some(origin) = storage["securityOrigin"].as_str() else {
        return;
    };
    let items = origins.entry(origin.to_string()).or_default();
    match method {
        "DOMStorage.domStorageItemsCleared" => items.clear(),
        "DOMStorage.domStorageItemRemoved" => {
            if let Some(name) = params["key"].as_str() {
                items.remove(name);
            }
        }
        "DOMStorage.domStorageItemAdded" | "DOMStorage.domStorageItemUpdated" => {
            if let (Some(name), Some(value)) = (params["key"].as_str(), params["newValue"].as_str())
            {
                items.insert(name.into(), value.into());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playwright_origins_validate_and_keep_same_named_keys_separate() {
        let value = json!({"origins": [
            {"origin": "https://one.example", "localStorage": [{"name": "token", "value": "first"}]},
            {"origin": "https://two.example", "localStorage": [{"name": "token", "value": "second"}]}
        ]});
        let origins = validate_origins(&value).unwrap();
        assert_eq!(origins["https://one.example"]["token"], "first");
        assert_eq!(origins["https://two.example"]["token"], "second");
        assert!(
            validate_origins(
                &json!({"origins": [{"origin": "https://one.example/path", "localStorage": []}]})
            )
            .is_err()
        );
        assert!(validate_origins(&json!({"origins": [{"origin": "https://one.example", "localStorage": [{"name": "token", "value": 1}]}]})).is_err());
    }

    #[tokio::test]
    #[ignore = "requires a locally installed Chromium browser"]
    async fn storage_is_restored_before_scripts_and_persisted_across_origins() -> Result<()> {
        use super::super::browser_tests::Site;
        use tiny_http::{Header, Response};
        let site = || {
            Site::new(|request| {
                let _ = request.respond(
                    Response::from_string(
                        "<script>window.initialToken = localStorage.getItem('token');</script>",
                    )
                    .with_header(Header::from_bytes("Content-Type", "text/html").unwrap()),
                );
            })
        };
        let first = site()?;
        let second = site()?;
        let (mut browser, handler, directory) =
            super::super::browsers::launch_browser(&None, "state", Default::default())
                .await
                .map_err(|error| anyhow!("{error}"))?;
        let result: Result<()> = async {
            let page = browser.new_page("about:blank").await?;
            let initial = Origins::from([(
                first.origin.clone(),
                BTreeMap::from([("token".into(), "restored".into())]),
            )]);
            let tracker = StorageState::install_origins(&browser, &page, initial).await?;
            page.goto(first.origin.as_str()).await?;
            let observed: String = page.evaluate("window.initialToken").await?.into_value()?;
            anyhow::ensure!(
                observed == "restored",
                "application did not see restored state on first load"
            );
            page.evaluate("localStorage.setItem('token', 'changed')")
                .await?;
            page.evaluate("document.cookie = 'first=one; path=/'")
                .await?;
            page.goto(second.origin.as_str()).await?;
            page.evaluate("localStorage.setItem('token', 'second')")
                .await?;
            page.evaluate("document.cookie = 'second=two; path=/'")
                .await?;
            page.goto(first.origin.as_str()).await?;
            let captured = tracker.capture(&page).await?;
            anyhow::ensure!(
                captured["cookies"].as_array().is_some_and(|cookies| {
                    cookies.iter().any(|cookie| cookie["name"] == "first")
                        && cookies.iter().any(|cookie| cookie["name"] == "second")
                }),
                "browser cookies were not all persisted"
            );
            let origins = validate_origins(&captured)?;
            anyhow::ensure!(
                origins[&first.origin]["token"] == "changed",
                "restoration overwrote changed storage"
            );
            anyhow::ensure!(
                origins
                    .get(&second.origin)
                    .and_then(|items| items.get("token"))
                    .map(String::as_str)
                    == Some("second"),
                "previous origin storage missing"
            );
            Ok(())
        }
        .await;
        browser.close().await?;
        handler.await?;
        let _ = std::fs::remove_dir_all(directory);
        result
    }
}
