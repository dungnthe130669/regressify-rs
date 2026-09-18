use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow};
use tiny_http::{Header, Request, Response, Server, StatusCode};

use crate::types::BasicAuth;

use super::{
    authentication::Authentication,
    browsers::{LaunchOptions, launch_browser},
};

pub(super) struct Site {
    pub origin: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Site {
    pub fn new(handler: impl Fn(Request) + Send + 'static) -> Result<Self> {
        let server = Server::http("127.0.0.1:0").map_err(|error| anyhow!("{error}"))?;
        let origin = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(request)) => handler(request),
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            origin,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn auth_site(
    expected: &'static str,
    credentials_seen: Arc<AtomicBool>,
    redirect: Option<String>,
) -> Result<Site> {
    Site::new(move |request| {
        if request.url() == "/redirect"
            && let Some(location) = &redirect
        {
            let response = Response::from_string("")
                .with_status_code(StatusCode(302))
                .with_header(Header::from_bytes("Location", location.as_str()).unwrap());
            let _ = request.respond(response);
            return;
        }
        let authorization = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Authorization"));
        if authorization.is_some() {
            credentials_seen.store(true, Ordering::Relaxed);
        }
        let response = if authorization.is_some_and(|header| header.value.as_str() == expected) {
            Response::from_string("<body>authenticated</body>")
        } else {
            Response::from_string("<body>unauthorized</body>")
                .with_status_code(StatusCode(401))
                .with_header(
                    Header::from_bytes("WWW-Authenticate", "Basic realm=\"fixture\"").unwrap(),
                )
        };
        let _ =
            request.respond(response.with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
    })
}

#[tokio::test]
#[ignore = "requires a locally installed Chromium browser"]
async fn basic_auth_is_origin_scoped_and_cancels_bad_credentials() -> Result<()> {
    let allowed = Arc::new(AtomicBool::new(false));
    let other_allowed = Arc::new(AtomicBool::new(false));
    let forbidden = Arc::new(AtomicBool::new(false));
    let third = auth_site("Basic dXNlcjpwYXNz", forbidden.clone(), None)?;
    let first = auth_site(
        "Basic dXNlcjpwYXNz",
        allowed.clone(),
        Some(third.origin.clone()),
    )?;
    let second = auth_site("Basic b3RoZXI6cGFzcw==", other_allowed.clone(), None)?;
    let (mut browser, handler, directory) =
        launch_browser(&None, "fixture", LaunchOptions::default())
            .await
            .map_err(|error| anyhow!("{error}"))?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let entries = [
            BasicAuth {
                origin: first.origin.clone(),
                username: "user".into(),
                password: "pass".into(),
            },
            BasicAuth {
                origin: second.origin.clone(),
                username: "other".into(),
                password: "pass".into(),
            },
        ];
        let authentication = Authentication::install(&browser, &page, &entries)
            .await?
            .unwrap();
        for site in [&first, &second] {
            page.goto(site.origin.as_str()).await?;
            let text: String = page
                .evaluate("document.body.textContent")
                .await?
                .into_value()?;
            anyhow::ensure!(
                text == "authenticated",
                "unexpected authentication response: {text}"
            );
        }
        let _ =
            tokio::time::timeout(Duration::from_secs(10), page.goto(third.origin.as_str())).await?;
        let _ = tokio::time::timeout(
            Duration::from_secs(10),
            page.goto(format!("{}/redirect", first.origin)),
        )
        .await?;
        anyhow::ensure!(
            !forbidden.load(Ordering::Relaxed),
            "credentials leaked to an unconfigured origin"
        );
        anyhow::ensure!(
            allowed.load(Ordering::Relaxed),
            "first origin did not authenticate"
        );
        anyhow::ensure!(
            other_allowed.load(Ordering::Relaxed),
            "second origin did not authenticate"
        );
        authentication.check()?;
        let wrong_page = browser.new_page("about:blank").await?;
        let _wrong_authentication = Authentication::install(
            &browser,
            &wrong_page,
            &[BasicAuth {
                origin: third.origin.clone(),
                username: "wrong".into(),
                password: "wrong".into(),
            }],
        )
        .await?;
        let _ = tokio::time::timeout(
            Duration::from_secs(10),
            wrong_page.goto(third.origin.as_str()),
        )
        .await?;
        Ok(())
    }
    .await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}

fn interaction_page(request: Request) {
    let content = match request.url() {
        "/done" => "<body>done<script src='/slow.js'></script></body>",
        "/slow.js" => {
            std::thread::sleep(Duration::from_millis(150));
            "document.body.dataset.loaded = 'yes';"
        }
        _ => {
            r#"<input id="typed" maxlength="3"><select id="choice"><option value="old">Old</option></select>
            <a id="next" href="/done">Next</a><script>
            const typed = document.querySelector('#typed');
            const keys = [];
            typed.addEventListener('keydown', event => { keys.push(event.key); typed.dataset.keys = JSON.stringify(keys); typed.dataset.trusted = String(event.isTrusted); });
            setTimeout(() => document.querySelector('#choice').add(new Option('Ready', 'ready')), 600);
            </script>"#
        }
    };
    let mime = if request.url() == "/slow.js" {
        "application/javascript"
    } else {
        "text/html"
    };
    let _ = request.respond(
        Response::from_string(content)
            .with_header(Header::from_bytes("Content-Type", mime).unwrap()),
    );
}

#[tokio::test]
#[ignore = "requires a locally installed Chromium browser"]
async fn action_keyboard_selection_and_navigation_work_in_frames() -> Result<()> {
    use super::{
        runtime::Runtime,
        storage_state::StorageState,
        test_action::{ActionExecutionContext, exec_test_action},
    };
    use crate::{
        config::ResolvedConfig,
        types::{Frame, TestAction, TestScenario, TestSuite},
    };
    let inner = Site::new(interaction_page)?;
    let outer = Site::new(interaction_page)?;
    let (mut browser, handler, directory) =
        launch_browser(&None, "interactions", LaunchOptions::default())
            .await
            .map_err(|error| anyhow!("{error}"))?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = Runtime::new(&browser, &page).await?;
        let storage = StorageState::install(&browser, &page, None, &outer.origin).await?;
        page.goto(outer.origin.as_str()).await?;
        page.evaluate(format!("(() => {{ const child = document.createElement('iframe'); child.src = {}; document.body.append(child); }})()", serde_json::to_string(&inner.origin.replace("127.0.0.1", "localhost"))?)).await?;
        let suite = TestSuite::default();
        let scenario = TestScenario { url: outer.origin.clone(), ..Default::default() };
        let config = ResolvedConfig { element_timeout_ms: 4000, actionability_timeout_ms: 2000, ..Default::default() };
        let context = || ActionExecutionContext { page: &page, runtime: &runtime, storage: &storage, test_suite: &suite, scenario: &scenario, config: &config, url_replacements: &[], viewport_label: None, log_prefix: "fixture" };
        for frame in [None, Some(Frame::Single("iframe".into()))] {
            for action in [
                serde_json::json!({"select":"#choice", "value":"ready", "frame":frame}),
                serde_json::json!({"input":"#typed", "value":"Ab+c", "append":true, "frame":frame}),
            ] {
                let action: TestAction = serde_json::from_value(action)?;
                exec_test_action(context(), &action).await?;
            }
            let target = runtime.target(&frame, Duration::from_secs(4)).await?;
            let values: serde_json::Value = target.evaluate("(() => { const input = document.querySelector('#typed'); return {value:input.value, keys:JSON.parse(input.dataset.keys), trusted:input.dataset.trusted, choice:document.querySelector('#choice').value}; })()").await?.into_value()?;
            anyhow::ensure!(values["value"] == "Ab+", "maxlength was bypassed: {values}");
            anyhow::ensure!(values["keys"] == serde_json::json!(["A", "b", "+", "c"]), "typing did not dispatch per-character key events: {values}");
            anyhow::ensure!(values["trusted"] == "true" && values["choice"] == "ready", "trusted typing or delayed options failed");
            for key in ["Shift", "ControlOrMeta+A", "Backspace", "+"] {
                let action: TestAction = serde_json::from_value(serde_json::json!({"press":"#typed", "key":key, "frame":frame}))?;
                exec_test_action(context(), &action).await?;
            }
            let value: String = target.evaluate("document.querySelector('#typed').value").await?.into_value()?;
            anyhow::ensure!(value == "+", "keyboard shortcut or literal plus failed: {value}");
        }
        let action: TestAction = serde_json::from_value(serde_json::json!({"click":"#next"}))?;
        exec_test_action(context(), &action).await?;
        let loaded: String = page.evaluate("document.body.dataset.loaded").await?.into_value()?;
        anyhow::ensure!(loaded == "yes", "click completed before the navigation loaded");
        Ok(())
    }.await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}
