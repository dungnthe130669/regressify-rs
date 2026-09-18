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

struct Site {
    origin: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Site {
    fn new(handler: impl Fn(Request) + Send + 'static) -> Result<Self> {
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
