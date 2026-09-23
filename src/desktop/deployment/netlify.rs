use std::{path::Path, time::Duration};

use anyhow::{Context, Result, anyhow, bail, ensure};
use futures::StreamExt;
use reqwest::{
    Client, Url,
    header::{AUTHORIZATION, HeaderValue},
};
use serde::{Deserialize, de::DeserializeOwned};

use super::{DeployTarget, DeploymentProvider, diagnostics, package::Package, phase};

const UPLOAD_PROGRESS_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
struct Site {
    id: String,
    prevent_non_git_prod_deploys: Option<bool>,
    published_deploy: Option<PublishedDeploy>,
}

#[derive(Deserialize)]
struct PublishedDeploy {
    locked: Option<bool>,
}

#[derive(Deserialize)]
struct Deploy {
    id: String,
    site_id: String,
    state: String,
    ssl_url: Option<String>,
    deploy_ssl_url: Option<String>,
    draft: Option<bool>,
    #[serde(default)]
    error_message: serde_json::Value,
    #[serde(default)]
    error: serde_json::Value,
}

pub struct NetlifyClient {
    client: Client,
    endpoint: Url,
    authorization: HeaderValue,
    backoff: Duration,
    poll_interval: Duration,
    deadline: Duration,
}

impl NetlifyClient {
    pub fn new(token: &str) -> Result<Self> {
        DeploymentProvider::Netlify.validate_token(token)?;
        let value = zeroize::Zeroizing::new(format!("Bearer {token}"));
        let mut authorization = HeaderValue::from_str(&value)
            .map_err(|_| anyhow!("Invalid Netlify personal access token"))?;
        authorization.set_sensitive(true);
        Ok(Self {
            client: Client::builder()
                .user_agent(concat!("Regressify-Desktop/", env!("CARGO_PKG_VERSION")))
                .redirect(reqwest::redirect::Policy::none())
                .https_only(!cfg!(test))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(180))
                .build()?,
            endpoint: Url::parse("https://api.netlify.com/api/v1/")?,
            authorization,
            backoff: Duration::from_secs(1),
            poll_interval: Duration::from_secs(5),
            deadline: Duration::from_secs(60 * 60),
        })
    }

    async fn get<Response: DeserializeOwned>(&self, path: &str) -> Result<Response> {
        let url = self.endpoint.join(path)?;
        for attempt in 0..6 {
            let response = self
                .client
                .get(url.clone())
                .header(AUTHORIZATION, self.authorization.clone())
                .timeout(Duration::from_secs(30))
                .send()
                .await;
            let mut retry_after = None;
            let error = match response {
                Ok(response) if response.status().is_success() => return read_json(response).await,
                Ok(response) => {
                    let status = response.status();
                    retry_after = response
                        .headers()
                        .get("Retry-After")
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(|seconds| Duration::from_secs(seconds.min(60)));
                    let error = diagnostics::http_error(
                        response,
                        DeploymentProvider::Netlify,
                        &self.authorization,
                    )
                    .await;
                    if !status.is_server_error() && !matches!(status.as_u16(), 408 | 429) {
                        return Err(error);
                    }
                    error
                }
                Err(_) => anyhow!("Cannot reach Netlify; check your connection and try again"),
            };
            if attempt == 5 {
                return Err(error);
            }
            let delay = retry_after.unwrap_or(self.backoff * (1 << attempt));
            phase(&format!(
                "Netlify retry {}/6 in {:.1}s: {error}",
                attempt + 2,
                delay.as_secs_f64()
            ));
            tokio::time::sleep(delay).await;
        }
        bail!("Netlify request retry budget exhausted")
    }

    pub async fn deploy(&self, root: &Path, target: &DeployTarget) -> Result<String> {
        target.validate()?;
        ensure!(
            target.provider == DeploymentProvider::Netlify,
            "Netlify client requires a Netlify target"
        );
        let operation = async {
            validate_static_configuration(root)?;
            phase("Validating Netlify deployment target");
            let site_path = format!("sites/{}", target.site_id);
            let site: Site = self.get(&site_path).await?;
            ensure!(
                site.id.eq_ignore_ascii_case(&target.site_id),
                "Netlify returned a different site"
            );
            ensure!(
                site.prevent_non_git_prod_deploys != Some(true),
                "This Netlify site only allows production deploys from Git"
            );
            ensure!(
                !site
                    .published_deploy
                    .is_some_and(|deploy| deploy.locked == Some(true)),
                "Unlock the current Netlify deployment before publishing reports"
            );
            phase("Packaging selected reports for Netlify");
            let directory = root.to_path_buf();
            let package =
                tokio::task::spawn_blocking(move || Package::create(&directory, 25_000)).await??;
            phase(&format!(
                "Uploading {} report files to Netlify",
                package.files
            ));
            let input = tokio::fs::File::open(package.file.path()).await?;
            let length = package.file.as_file().metadata()?.len();
            let sent = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
            let streamed = sent.clone();
            let body = futures::stream::try_unfold(
                (input, std::time::Instant::now()),
                move |(mut input, mut last_update)| {
                    let streamed = streamed.clone();
                    async move {
                        use tokio::io::AsyncReadExt;
                        let mut buffer = vec![0; 64 * 1024];
                        let count = input.read(&mut buffer).await?;
                        if count == 0 {
                            return Ok::<_, std::io::Error>(None);
                        }
                        buffer.truncate(count);
                        let completed = streamed
                            .fetch_add(count as u64, std::sync::atomic::Ordering::Relaxed)
                            + count as u64;
                        if completed == length || last_update.elapsed().as_secs() >= 1 {
                            phase(&format!(
                                "Netlify upload sent: {} of {} KiB ({:.1}%); awaiting provider acceptance",
                                completed / 1024,
                                length / 1024,
                                completed as f64 * 100.0 / length as f64
                            ));
                            last_update = std::time::Instant::now();
                        }
                        Ok(Some((buffer, (input, last_update))))
                    }
                },
            );
            let upload = self
                .client
                .post(self.endpoint.join(&format!("{site_path}/deploys"))?)
                .header(AUTHORIZATION, self.authorization.clone())
                .header("Content-Type", "application/zip")
                .header("Content-Length", length)
                .timeout(self.deadline)
                .body(reqwest::Body::wrap_stream(body))
                .send();
            let mut progress = UploadStallMonitor::default();
            let response = super::monitor(upload, UPLOAD_PROGRESS_INTERVAL, |elapsed| {
                if let Some(warning) = progress.warning(
                    sent.load(std::sync::atomic::Ordering::Relaxed), length, elapsed,
                ) {
                    phase(&warning);
                }
            }).await
                .map_err(|error| anyhow!("Netlify upload {} after sending {} of {} KiB; check the site's deploy history before retrying",
                    if error.is_timeout() { "timed out" } else if error.is_connect() { "could not connect" } else { "did not complete" },
                    sent.load(std::sync::atomic::Ordering::Relaxed) / 1024, length / 1024))?;
            if !response.status().is_success() {
                let error = diagnostics::http_error(
                    response,
                    DeploymentProvider::Netlify,
                    &self.authorization,
                )
                .await;
                bail!("{error}; check the site's deploy history before retrying");
            }
            let mut deploy: Deploy = read_json(response).await?;
            ensure!(
                !deploy.id.is_empty()
                    && deploy.id.len() <= 128
                    && deploy
                        .id
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
                "Netlify returned an invalid deployment ID"
            );
            let deploy_id = deploy.id.clone();
            phase(&format!(
                "Netlify accepted upload; waiting for deployment {deploy_id}"
            ));
            let polling_started = std::time::Instant::now();
            let mut polls = 0;
            loop {
                ensure!(
                    deploy.id == deploy_id && deploy.site_id.eq_ignore_ascii_case(&target.site_id),
                    "Netlify returned a different deployment"
                );
                ensure!(
                    deploy.draft != Some(true),
                    "Netlify returned a draft instead of a production deployment"
                );
                match deploy.state.as_str() {
                    "ready" => {
                        let value = deploy
                            .ssl_url
                            .as_deref()
                            .filter(|value| !value.is_empty())
                            .or(deploy.deploy_ssl_url.as_deref())
                            .context("Netlify deployment is ready without an HTTPS site URL")?;
                        let url = site_url(value)?;
                        phase(&format!("Deployed: {url}"));
                        return Ok(url);
                    }
                    "error" | "failed" | "canceled" | "cancelled" | "rejected" => {
                        let error = diagnostics::failure(
                            DeploymentProvider::Netlify,
                            "deployment",
                            None,
                            &serde_json::json!({"error_message": deploy.error_message, "error": deploy.error}),
                            &self.authorization,
                        );
                        bail!("{error} Deployment ID: {deploy_id}");
                    }
                    "new" | "pending_review" | "accepted" | "enqueued" | "building"
                    | "uploading" | "uploaded" | "preparing" | "prepared" | "processing"
                    | "processed" | "retrying" => {}
                    _ => bail!(
                        "Netlify returned an unexpected deployment state; check the site's deploy history"
                    ),
                }
                polls += 1;
                phase(&format!(
                    "Netlify status: {}; poll {polls}, {}s waiting; next check in {:.1}s",
                    deploy.state,
                    polling_started.elapsed().as_secs(),
                    self.poll_interval.as_secs_f64()
                ));
                tokio::time::sleep(self.poll_interval).await;
                deploy = self
                    .get(&format!("{site_path}/deploys/{deploy_id}"))
                    .await?;
            }
        };
        tokio::time::timeout(
            self.deadline,
            super::with_heartbeat("Netlify deployment", operation),
        )
        .await
        .context(
            "Netlify deployment exceeded its 1-hour deadline; check the site's deploy history",
        )?
    }
}

#[derive(Default)]
struct UploadStallMonitor {
    last_sent: u64,
    last_progress: Duration,
}

impl UploadStallMonitor {
    fn warning(&mut self, sent: u64, length: u64, elapsed: Duration) -> Option<String> {
        if sent != self.last_sent {
            self.last_sent = sent;
            self.last_progress = elapsed;
            return None;
        }
        let idle = elapsed.saturating_sub(self.last_progress);
        if idle < UPLOAD_PROGRESS_INTERVAL {
            return None;
        }
        // Body consumption measures local progress, not Netlify's acceptance.
        let status = if sent == length {
            format!(
                "Netlify upload sent all {} KiB; still awaiting provider acceptance after {}s",
                length / 1024,
                idle.as_secs()
            )
        } else {
            format!(
                "Netlify upload has made no progress for {}s ({} of {} KiB sent)",
                idle.as_secs(),
                sent / 1024,
                length / 1024
            )
        };
        Some(format!(
            "{status}. Keep waiting or choose Stop to cancel locally."
        ))
    }
}

async fn read_json<Response: DeserializeOwned>(response: reqwest::Response) -> Result<Response> {
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| anyhow!("Cannot read Netlify response"))?;
        ensure!(
            bytes.len() + chunk.len() <= 1024 * 1024,
            "Netlify response exceeds 1 MiB"
        );
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid Netlify response"))
}

fn site_url(value: &str) -> Result<String> {
    let url = Url::parse(value).map_err(|_| anyhow!("Netlify returned an invalid site URL"))?;
    ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Netlify returned an unsafe site URL"
    );
    Ok(url.to_string())
}

fn validate_static_configuration(root: &Path) -> Result<()> {
    let path = root.join("staticwebapp.config.json");
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("Cannot read Azure static site configuration"),
    };
    let config: serde_json::Value = serde_json::from_slice(&super::bounded_read(
        file,
        1024 * 1024,
        "Static site configuration",
    )?)
    .map_err(|_| anyhow!("Invalid Azure static site configuration"))?;
    let restricted = config.get("auth").is_some()
        || config.get("networking").is_some()
        || config["routes"].as_array().is_some_and(|routes| {
            routes.iter().any(|route| {
                route.get("allowedRoles").is_some_and(|roles| {
                    !roles
                        .as_array()
                        .is_some_and(|roles| roles.iter().any(|role| role == "anonymous"))
                })
            })
        });
    ensure!(
        !restricted,
        "Azure access restrictions cannot be enforced by Netlify; configure Netlify access protection before deploying these reports"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::{Cursor, Read},
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    const SITE_ID: &str = "eacbea58-4222-4e16-9611-f3b91082a8aa";
    const TOKEN: &str = "synthetic-netlify-access-token";

    fn target() -> DeployTarget {
        DeployTarget {
            id: "netlify".into(),
            name: "Netlify QA".into(),
            credential_id: None,
            environment: String::new(),
            provider: DeploymentProvider::Netlify,
            site_id: SITE_ID.into(),
        }
    }

    fn client(origin: &str) -> NetlifyClient {
        let mut client = NetlifyClient::new(TOKEN).unwrap();
        client.endpoint = Url::parse(&format!("{origin}/api/v1/")).unwrap();
        client.backoff = Duration::from_millis(1);
        client.poll_interval = Duration::from_millis(1);
        client
    }

    #[tokio::test]
    #[ignore = "requires NETLIFY_PAT and NETLIFY_SITE_ID; validates site access without publishing"]
    async fn live_deployment_netlify_preflight() -> Result<()> {
        let token = zeroize::Zeroizing::new(
            std::env::var("NETLIFY_PAT").context("Set NETLIFY_PAT for live validation")?,
        );
        let mut target = target();
        target.site_id =
            std::env::var("NETLIFY_SITE_ID").context("Set NETLIFY_SITE_ID for live validation")?;
        target.validate()?;
        let client = NetlifyClient::new(&token)?;
        let site: Site = client.get(&format!("sites/{}", target.site_id)).await?;
        ensure!(
            site.id.eq_ignore_ascii_case(&target.site_id),
            "Netlify returned a different site"
        );
        println!(
            "Live Netlify validation succeeded: Git-only restriction {}, locked deployment {}; no deployment created",
            site.prevent_non_git_prod_deploys == Some(true),
            site.published_deploy
                .is_some_and(|deploy| deploy.locked == Some(true))
        );
        Ok(())
    }

    #[tokio::test]
    async fn zip_deployment_authenticates_retries_reads_and_waits_for_ready() -> Result<()> {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let reads = Arc::new(AtomicUsize::new(0));
        let attempts = reads.clone();
        let site = crate::test::browser_tests::Site::new(move |mut request| {
            let path = request.url().to_owned();
            let method = request.method().as_str().to_owned();
            let auth = request
                .headers()
                .iter()
                .find(|header| header.field.equiv("Authorization"))
                .map(|header| header.value.to_string());
            assert_eq!(auth.as_deref(), Some(format!("Bearer {TOKEN}").as_str()));
            let content_type = request
                .headers()
                .iter()
                .find(|header| header.field.equiv("Content-Type"))
                .map(|header| header.value.to_string());
            let mut body = Vec::new();
            request.as_reader().read_to_end(&mut body).unwrap();
            captured
                .lock()
                .unwrap()
                .push((method.clone(), path.clone(), content_type, body));
            let response = if path == format!("/api/v1/sites/{SITE_ID}") {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    let _ = request.respond(tiny_http::Response::empty(503));
                    return;
                }
                json!({"id": SITE_ID, "prevent_non_git_prod_deploys": null,
                    "published_deploy": {"locked": null}})
            } else {
                json!({"id":"deploy-123", "site_id": SITE_ID,
                    "state": if method == "POST" { "processing" } else { "ready" },
                    "draft": null,
                    "ssl_url": "https://qa-reports.netlify.app/"})
            };
            let _ = request.respond(tiny_http::Response::from_string(response.to_string()));
        })?;
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("output");
        std::fs::create_dir_all(output.join("one/html_report"))?;
        std::fs::write(output.join("index.html"), "report")?;
        std::fs::write(output.join("one/html_report/index.html"), "suite report")?;
        let report = json!({"id": "one", "tests": []});
        std::fs::write(
            output.join("one/html_report/config.js"),
            format!("report({report});"),
        )?;
        let headers = include_str!("../../snapshot/data/_headers");
        std::fs::write(output.join("_headers"), headers)?;
        std::fs::write(
            output.join("staticwebapp.config.json"),
            include_bytes!("../../snapshot/data/staticwebapp.config.json"),
        )?;
        let root = directory.path().join("publish");
        super::super::stage_reports(&output, &["one".into()], &root)?;
        assert_eq!(
            client(&site.origin).deploy(&root, &target()).await?,
            "https://qa-reports.netlify.app/"
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        let records = requests.lock().unwrap();
        let uploads = records
            .iter()
            .filter(|record| record.0 == "POST")
            .collect::<Vec<_>>();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].1, format!("/api/v1/sites/{SITE_ID}/deploys"));
        assert_eq!(uploads[0].2.as_deref(), Some("application/zip"));
        let mut zip = zip::ZipArchive::new(Cursor::new(&uploads[0].3))?;
        assert_eq!(zip.by_name("index.html")?.size(), 6);
        assert_eq!(zip.by_name("one/html_report/index.html")?.size(), 12);
        let mut uploaded_headers = String::new();
        zip.by_name("_headers")?
            .read_to_string(&mut uploaded_headers)?;
        assert_eq!(uploaded_headers, headers);
        assert!(
            records
                .iter()
                .any(|record| record.1 == format!("/api/v1/sites/{SITE_ID}/deploys/deploy-123"))
        );
        assert!(records.iter().all(|record| !record.1.contains(TOKEN)
            && !String::from_utf8_lossy(&record.3).contains(TOKEN)));
        Ok(())
    }

    #[tokio::test]
    async fn upload_can_outlast_the_default_request_timeout() -> Result<()> {
        let uploads = Arc::new(AtomicUsize::new(0));
        let attempts = uploads.clone();
        let site = crate::test::browser_tests::Site::new(move |mut request| {
            let response = if request.method().as_str() == "POST" {
                attempts.fetch_add(1, Ordering::SeqCst);
                std::io::copy(request.as_reader(), &mut std::io::sink()).unwrap();
                std::thread::sleep(Duration::from_millis(300));
                json!({"id":"deploy-123", "site_id":SITE_ID, "state":"ready",
                    "ssl_url":"https://qa-reports.netlify.app/"})
            } else {
                json!({"id":SITE_ID})
            };
            let _ = request.respond(tiny_http::Response::from_string(response.to_string()));
        })?;
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("index.html"), "report")?;
        let mut client = client(&site.origin);
        let default_deadline = client.deadline;
        client.client = Client::builder()
            .timeout(Duration::from_millis(25))
            .build()?;
        client.deadline = Duration::from_secs(5);
        assert_eq!(
            client.deploy(root.path(), &target()).await?,
            "https://qa-reports.netlify.app/"
        );
        assert_eq!(uploads.load(Ordering::SeqCst), 1);
        assert_eq!(default_deadline, Duration::from_secs(60 * 60));
        client.deadline = Duration::from_millis(150);
        let error = client
            .deploy(root.path(), &target())
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("deadline") || error.contains("timed out"),
            "{error}"
        );
        assert_eq!(uploads.load(Ordering::SeqCst), 2);
        Ok(())
    }

    #[test]
    fn upload_stall_warnings_repeat_and_reset_when_progress_resumes() {
        let mut progress = UploadStallMonitor::default();
        let length = 4 * 1024;
        for seconds in [0, 59] {
            assert!(
                progress
                    .warning(0, length, Duration::from_secs(seconds))
                    .is_none()
            );
        }
        for seconds in [60, 120] {
            let warning = progress
                .warning(0, length, Duration::from_secs(seconds))
                .unwrap();
            assert!(
                warning.contains(&format!("no progress for {seconds}s")),
                "{warning}"
            );
            assert!(warning.contains("0 of 4 KiB sent"), "{warning}");
            assert!(warning.contains("Keep waiting or choose Stop"), "{warning}");
        }
        assert!(
            progress
                .warning(2 * 1024, length, Duration::from_secs(130))
                .is_none()
        );
        assert!(
            progress
                .warning(2 * 1024, length, Duration::from_secs(189))
                .is_none()
        );
        let warning = progress
            .warning(2 * 1024, length, Duration::from_secs(190))
            .unwrap();
        assert!(warning.contains("no progress for 60s"), "{warning}");
        assert!(warning.contains("2 of 4 KiB sent"), "{warning}");
        assert!(
            progress
                .warning(3 * 1024, length, Duration::from_secs(200))
                .is_none()
        );
    }

    #[test]
    fn completed_upload_waits_for_acceptance_without_claiming_a_stalled_transfer() {
        let mut progress = UploadStallMonitor::default();
        let length = 4 * 1024;
        assert!(
            progress
                .warning(length, length, Duration::from_secs(1))
                .is_none()
        );
        assert!(
            progress
                .warning(length, length, Duration::from_secs(60))
                .is_none()
        );
        for seconds in [61, 121] {
            let warning = progress
                .warning(length, length, Duration::from_secs(seconds))
                .unwrap();
            assert!(warning.contains("sent all 4 KiB"), "{warning}");
            assert!(
                warning.contains(&format!(
                    "awaiting provider acceptance after {}s",
                    seconds - 1
                )),
                "{warning}"
            );
            assert!(!warning.contains("no progress"), "{warning}");
            assert!(warning.contains("Keep waiting or choose Stop"), "{warning}");
        }
    }

    #[tokio::test]
    async fn failed_upload_is_not_retried_and_errors_are_redacted() -> Result<()> {
        let uploads = Arc::new(AtomicUsize::new(0));
        let attempts = uploads.clone();
        let site = crate::test::browser_tests::Site::new(move |mut request| {
            if request.method().as_str() == "POST" {
                attempts.fetch_add(1, Ordering::SeqCst);
                std::io::copy(request.as_reader(), &mut std::io::sink()).unwrap();
                let _ = request.respond(tiny_http::Response::from_string(
                    json!({"message":format!("Upload exceeds the project size limit; token={TOKEN}")}).to_string()
                ).with_status_code(503).with_header(
                    tiny_http::Header::from_bytes("x-nf-request-id", "request-123").unwrap()
                ));
            } else {
                let _ = request.respond(tiny_http::Response::from_string(
                    json!({"id":SITE_ID}).to_string(),
                ));
            }
        })?;
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("index.html"), "report")?;
        let error = client(&site.origin)
            .deploy(root.path(), &target())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("HTTP 503"));
        assert!(
            error
                .to_string()
                .contains("Upload exceeds the project size limit")
        );
        assert!(error.to_string().contains("request-123"));
        assert!(!format!("{error:#}").contains(TOKEN));
        assert_eq!(uploads.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test]
    async fn authentication_redirects_and_oversized_responses_fail_before_upload() -> Result<()> {
        for (status, body) in [
            (401, TOKEN.to_string()),
            (403, TOKEN.to_string()),
            (404, TOKEN.to_string()),
            (302, TOKEN.to_string()),
            (200, TOKEN.to_string()),
            (200, "x".repeat(1024 * 1024 + 1)),
            (413, "x".repeat(1024 * 1024 + 1)),
            (200, json!({"id":"wrong-site"}).to_string()),
            (
                200,
                json!({"id":SITE_ID, "prevent_non_git_prod_deploys":true,
                    "published_deploy":{"locked":null}})
                .to_string(),
            ),
            (
                200,
                json!({"id":SITE_ID, "prevent_non_git_prod_deploys":null,
                    "published_deploy":{"locked":true}})
                .to_string(),
            ),
            (
                200,
                json!({"id":SITE_ID, "prevent_non_git_prod_deploys":TOKEN}).to_string(),
            ),
            (
                200,
                json!({"id":SITE_ID, "published_deploy":{"locked":TOKEN}}).to_string(),
            ),
        ] {
            let calls = Arc::new(AtomicUsize::new(0));
            let attempts = calls.clone();
            let site = crate::test::browser_tests::Site::new(move |request| {
                attempts.fetch_add(1, Ordering::SeqCst);
                assert_eq!(request.method().as_str(), "GET");
                let response = tiny_http::Response::from_string(body.clone())
                    .with_status_code(status)
                    .with_header(
                        tiny_http::Header::from_bytes("Location", "https://example.com/leak")
                            .unwrap(),
                    );
                let _ = request.respond(response);
            })?;
            let root = tempfile::tempdir()?;
            std::fs::write(root.path().join("index.html"), "report")?;
            let error = client(&site.origin)
                .deploy(root.path(), &target())
                .await
                .unwrap_err();
            assert!(!format!("{error:#}").contains(TOKEN));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
        Ok(())
    }

    #[tokio::test]
    async fn polling_fails_for_terminal_states_changed_identity_and_deadlines() -> Result<()> {
        for (state, site_id, draft, expected) in [
            ("error", SITE_ID, json!(false), "deployment failed"),
            ("rejected", SITE_ID, json!(null), "deployment failed"),
            ("processing", SITE_ID, json!(false), "deadline"),
            ("ready", "wrong-site", json!(false), "different deployment"),
            ("ready", SITE_ID, json!(true), "draft instead"),
            ("ready", SITE_ID, json!(TOKEN), "Invalid Netlify response"),
        ] {
            let site = crate::test::browser_tests::Site::new(move |mut request| {
                let response = if request.url() == format!("/api/v1/sites/{SITE_ID}") {
                    json!({"id":SITE_ID})
                } else {
                    std::io::copy(request.as_reader(), &mut std::io::sink()).unwrap();
                    json!({"id":"deploy-123", "site_id": site_id,
                        "state":state, "draft":draft, "error_message":format!("Image processing failed; token={TOKEN}"),
                        "ssl_url":"https://qa-reports.netlify.app/"})
                };
                let _ = request.respond(tiny_http::Response::from_string(response.to_string()));
            })?;
            let root = tempfile::tempdir()?;
            std::fs::write(root.path().join("index.html"), "report")?;
            let mut client = client(&site.origin);
            if state == "processing" {
                client.deadline = Duration::from_millis(150);
            }
            let error = client.deploy(root.path(), &target()).await.unwrap_err();
            assert!(!format!("{error:#}").contains(TOKEN));
            assert!(error.to_string().contains(expected));
            if matches!(state, "error" | "rejected") {
                assert!(error.to_string().contains("Image processing failed"));
                assert!(error.to_string().contains("deploy-123"));
            }
        }
        Ok(())
    }

    #[test]
    fn unsafe_urls_and_azure_access_rules_are_rejected() -> Result<()> {
        for url in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com/?token=secret",
            "javascript:alert(1)",
        ] {
            assert!(site_url(url).is_err());
        }
        assert_eq!(
            site_url("https://reports.example.com")?,
            "https://reports.example.com/"
        );
        let root = tempfile::tempdir()?;
        let path = root.path().join("staticwebapp.config.json");
        std::fs::write(
            &path,
            include_bytes!("../../snapshot/data/staticwebapp.config.json"),
        )?;
        assert!(validate_static_configuration(root.path()).is_ok());
        for config in [
            json!({"auth":{}}),
            json!({"networking":{}}),
            json!({"routes":[{"allowedRoles":["authenticated"]}]}),
        ] {
            std::fs::write(&path, config.to_string())?;
            assert!(validate_static_configuration(root.path()).is_err());
        }
        Ok(())
    }
}
