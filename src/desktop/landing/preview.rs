use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result, ensure};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use super::LandingApp;

pub struct LandingPreview {
    pub url: String,
    server: Arc<Server>,
    stopped: Arc<AtomicBool>,
}

impl LandingPreview {
    pub fn start(app: &LandingApp) -> Result<Self> {
        let mut files = BTreeMap::new();
        files.insert(
            "/".to_owned(),
            (app.render()?.into_bytes(), "text/html; charset=utf-8"),
        );
        for (name, bytes) in crate::snapshot::FRONTEND_FILES {
            if !name.starts_with("_snapshot/assets/") {
                continue;
            }
            let mime = if name.ends_with(".js") {
                "text/javascript; charset=utf-8"
            } else if name.ends_with(".css") {
                "text/css; charset=utf-8"
            } else if name.ends_with(".svg") {
                "image/svg+xml"
            } else if name.ends_with(".woff2") {
                "font/woff2"
            } else {
                anyhow::bail!("Unsupported landing preview asset");
            };
            files.insert(
                format!(
                    "/{name}?v={}",
                    crate::snapshot::get_asset_version(bytes, 20)
                ),
                (bytes.to_vec(), mime),
            );
        }
        let server = Arc::new(
            Server::http("127.0.0.1:0")
                .map_err(|_| anyhow::anyhow!("Cannot start the local landing preview"))?,
        );
        let address = server
            .server_addr()
            .to_ip()
            .context("Landing preview address unavailable")?;
        ensure!(
            address.ip().is_loopback(),
            "Landing preview must remain on loopback"
        );
        let host = address.to_string();
        let url = format!("http://{host}/");
        let stopped = Arc::new(AtomicBool::new(false));
        let serving = server.clone();
        let stop = stopped.clone();
        std::thread::Builder::new()
            .name("landing-preview".into())
            .spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    let Ok(request) = serving.recv() else {
                        break;
                    };
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let permitted = matches!(request.method(), Method::Get | Method::Head)
                        && request.headers().iter().any(|header| {
                            header.field.equiv("Host") && header.value.as_str() == host
                        });
                    let response = if let Some((bytes, mime)) =
                        permitted.then(|| files.get(request.url())).flatten()
                    {
                        Response::from_data(bytes.clone()).with_header(
                            Header::from_bytes("Content-Type", mime.as_bytes()).unwrap(),
                        )
                    } else {
                        Response::from_data(b"Not found".to_vec()).with_status_code(StatusCode(404))
                    };
                    let response = response
                        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
                        .with_header(
                            Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap(),
                        )
                        .with_header(Header::from_bytes("Referrer-Policy", "no-referrer").unwrap());
                    let _ = request.respond(response);
                }
            })?;
        Ok(Self {
            url,
            server,
            stopped,
        })
    }
}

impl Drop for LandingPreview {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.server.unblock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_exposes_only_generated_html_and_exact_versioned_assets() {
        let preview = LandingPreview::start(&LandingApp::default()).unwrap();
        let client = reqwest::blocking::Client::new();
        let page = client.get(&preview.url).send().unwrap();
        assert_eq!(page.status(), 200);
        assert!(page.text().unwrap().contains("data-regressify-landing"));
        for path in [
            "regressify.yaml",
            "deployment-tokens.json",
            "_snapshot/assets/app.js",
            "_snapshot/status.json",
            "../private.json",
        ] {
            assert_eq!(
                client
                    .get(format!("{}{path}", preview.url))
                    .send()
                    .unwrap()
                    .status(),
                404
            );
        }
        assert_eq!(
            client
                .get(&preview.url)
                .header("Host", "untrusted.example")
                .send()
                .unwrap()
                .status(),
            404
        );
        let (name, bytes) = crate::snapshot::FRONTEND_FILES
            .iter()
            .find(|(name, _)| name.ends_with(".js"))
            .unwrap();
        let response = client
            .get(format!(
                "{}{name}?v={}",
                preview.url,
                crate::snapshot::get_asset_version(bytes, 20)
            ))
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.bytes().unwrap().as_ref(), *bytes);
    }
}
