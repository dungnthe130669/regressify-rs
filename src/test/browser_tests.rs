use std::{
    path::PathBuf,
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
    browsers::{LaunchOptions, prepare_browser},
};

pub(crate) async fn launch_browser(
    log_prefix: &str,
    options: LaunchOptions,
) -> Result<(chromiumoxide::Browser, tokio::task::JoinHandle<()>, PathBuf)> {
    let executable = prepare_browser(Some(false)).await?;
    super::browsers::launch_browser(&Some(executable), log_prefix, options)
        .await
        .map_err(|error| anyhow!("{error}"))
}

pub(crate) struct Site {
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
#[ignore = "requires managed Chromium; run regressify install"]
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
        launch_browser("fixture", LaunchOptions::default()).await?;
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
#[ignore = "requires managed Chromium; run regressify install"]
async fn navigation_default_allows_slow_document_responses() -> Result<()> {
    let site = Site::new(|request| {
        match request.url() {
            "/slow" => std::thread::sleep(Duration::from_secs(31)),
            "/timeout" => std::thread::sleep(Duration::from_millis(250)),
            _ => {}
        }
        let _ = request.respond(
            Response::from_string("<body>ready</body>")
                .with_header(Header::from_bytes("Content-Type", "text/html").unwrap()),
        );
    })?;
    let (mut browser, handler, directory) =
        launch_browser("navigation-timeout", LaunchOptions::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = super::runtime::Runtime::new(&browser, &page).await?;
        let target = runtime.target(&None, Duration::from_secs(5)).await?;
        target
            .navigate(&format!("{}/slow", site.origin), &Default::default())
            .await?;
        let loaded: bool = target
            .evaluate("document.readyState === 'complete'")
            .await?
            .into_value()?;
        anyhow::ensure!(loaded, "navigation returned before the document loaded");

        let error = target
            .navigate(
                &format!("{}/timeout", site.origin),
                &crate::types::GotoParameters {
                    timeout: Some(25),
                    ..Default::default()
                },
            )
            .await
            .expect_err("explicit navigation timeout must remain bounded");
        let message = error.to_string();
        anyhow::ensure!(
            message == "Navigation exceeded 25ms"
                || message == "Timed out waiting for CDP Page.navigate",
            "unexpected navigation timeout error: {message}"
        );
        Ok(())
    }
    .await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn navigation_options_wait_for_load_and_network_idle() -> Result<()> {
    let site = Site::new(interaction_page)?;
    let (mut browser, handler, directory) =
        launch_browser("navigation-options", LaunchOptions::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = super::runtime::Runtime::new(&browser, &page).await?;
        let target = runtime.target(&None, Duration::from_secs(5)).await?;
        for wait in [
            crate::types::NavigationWait::Load,
            crate::types::NavigationWait::NetworkIdle,
        ] {
            target
                .navigate(
                    &format!("{}/done", site.origin),
                    &crate::types::GotoParameters {
                        wait_until: Some(wait),
                        timeout: Some(5000),
                        referer: Some(format!("{}/source", site.origin)),
                    },
                )
                .await?;
            let loaded: bool = target
                .evaluate("document.body.dataset.loaded === 'yes'")
                .await?
                .into_value()?;
            anyhow::ensure!(
                loaded,
                "navigation returned before external script completed"
            );
        }
        Ok(())
    }
    .await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
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
        launch_browser("interactions", LaunchOptions::default()).await?;
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
            let expected_url = if frame.is_some() {
                format!("{}/", inner.origin.replace("127.0.0.1", "localhost"))
            } else {
                format!("{}/", outer.origin)
            };
            anyhow::ensure!(target.url().await?.as_deref() == Some(expected_url.as_str()), "URL read did not use the selected frame");
            let action: TestAction = serde_json::from_value(serde_json::json!({"wait":"#typed", "url":expected_url, "frame":frame}))?;
            exec_test_action(context(), &action).await?;
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

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn url_waits_follow_delayed_redirects_and_same_document_navigation() -> Result<()> {
    use super::{
        runtime::Runtime,
        storage_state::StorageState,
        test_action::{ActionExecutionContext, exec_test_action},
    };
    use crate::{
        config::ResolvedConfig,
        types::{TestAction, TestScenario, TestSuite},
    };

    let destination = Site::new(|request| {
        let content = if request.url() == "/callback" {
            "<body><script>setTimeout(() => location.replace('/done?flow=login#signed-in'), 150);</script></body>"
        } else {
            "<body><div id='signed-in'>Signed in</div></body>"
        };
        let _ =
            request.respond(Response::from_string(content).with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
    })?;
    let destination_origin = destination.origin.replace("127.0.0.1", "localhost");
    let callback_url = format!("{destination_origin}/callback");
    let source = Site::new(move |request| {
        let response = if request.url() == "/authorize" {
            Response::from_string("")
                .with_status_code(StatusCode(302))
                .with_header(Header::from_bytes("Location", callback_url.as_str()).unwrap())
        } else {
            Response::from_string(
                "<body><button id='login' onclick=\"setTimeout(() => location.assign('/authorize'), 250)\">Log in</button></body>",
            )
        };
        let _ =
            request.respond(response.with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
    })?;
    let (mut browser, handler, directory) =
        launch_browser("url-waits", LaunchOptions::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = Runtime::new(&browser, &page).await?;
        let storage = StorageState::install(&browser, &page, None, &source.origin).await?;
        page.goto(source.origin.as_str()).await?;
        let suite = TestSuite::default();
        let scenario = TestScenario {
            url: source.origin.clone(),
            ..Default::default()
        };
        let config = ResolvedConfig {
            element_timeout_ms: 6000,
            actionability_timeout_ms: 2000,
            ..Default::default()
        };
        let context = || ActionExecutionContext {
            page: &page,
            runtime: &runtime,
            storage: &storage,
            test_suite: &suite,
            scenario: &scenario,
            config: &config,
            url_replacements: &[],
            viewport_label: None,
            log_prefix: "fixture",
        };
        for action in [
            serde_json::json!({"click": "#login"}),
            serde_json::json!({
                "wait": "#signed-in",
                "url": format!("{destination_origin}/done?flow=login#signed-in")
            }),
        ] {
            let action: TestAction = serde_json::from_value(action)?;
            exec_test_action(context(), &action).await?;
        }
        let target = runtime.target(&None, Duration::from_secs(5)).await?;
        target
            .evaluate("history.pushState({}, '', '?flow=spa#ready')")
            .await?;
        let action: TestAction = serde_json::from_value(serde_json::json!({
            "wait": "#signed-in",
            "url": format!("{destination_origin}/done?flow=spa#ready")
        }))?;
        exec_test_action(context(), &action).await?;

        let timeout_config = ResolvedConfig {
            element_timeout_ms: 300,
            ..Default::default()
        };
        let action: TestAction = serde_json::from_value(serde_json::json!({
            "wait": 0,
            "url": format!("{destination_origin}/never")
        }))?;
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            exec_test_action(
                ActionExecutionContext {
                    config: &timeout_config,
                    ..context()
                },
                &action,
            ),
        )
        .await?
        .err()
        .ok_or_else(|| anyhow!("URL wait unexpectedly matched a different destination"))?;
        anyhow::ensure!(
            error.to_string().contains("Timeout waiting for URL"),
            "unexpected URL wait error: {error}"
        );
        Ok(())
    }
    .await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}
