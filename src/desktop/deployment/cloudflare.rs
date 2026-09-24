use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures::{StreamExt, TryStreamExt, stream};
use reqwest::{
    Client, RequestBuilder, Response, Url,
    header::{AUTHORIZATION, HeaderValue},
    multipart,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

const MAX_FILES: usize = 100_000;
const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_CONTENT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_BUCKET_BYTES: u64 = 50 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 32 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const UPLOAD_CONCURRENCY: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudflareTarget {
    pub account_id: String,
    pub worker_name: String,
}

impl CloudflareTarget {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.account_id.len() == 32
                && self.account_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Enter a valid Cloudflare Account ID (32 hexadecimal characters)"
        );
        validate_dns_label(&self.worker_name, "Worker name")
    }
}

fn validate_dns_label(value: &str, label: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 63
            && !value.starts_with('-')
            && !value.ends_with('-')
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
        "Cloudflare {label} must contain 1-63 letters, digits, or hyphens and cannot start or end with a hyphen"
    );
    Ok(())
}

pub(super) fn validate_token(token: &str) -> Result<()> {
    ensure!(
        !token.is_empty()
            && token.len() <= 1024
            && token.bytes().all(|byte| byte.is_ascii_graphic()),
        "Enter a valid Cloudflare API token"
    );
    Ok(())
}

pub struct CloudflareClient {
    client: Client,
    endpoint: Url,
    authorization: HeaderValue,
    backoff: Duration,
    read_timeout: Duration,
    deadline: Duration,
    verify_interval: Duration,
    verify_attempts: usize,
    #[cfg(test)]
    verification_url: Option<Url>,
}

#[derive(Deserialize)]
struct ApiEnvelope<Value> {
    success: bool,
    result: Option<Value>,
    errors: Option<Vec<ApiError>>,
}

#[derive(Deserialize)]
struct ApiError {
    code: u64,
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct AccountSubdomain {
    subdomain: String,
}

#[derive(Deserialize)]
struct WorkerRoute {
    enabled: bool,
}

#[derive(Deserialize)]
struct UploadSession {
    jwt: String,
    buckets: Vec<Vec<String>>,
}

#[derive(Deserialize)]
struct UploadResult {
    jwt: Option<String>,
}

#[derive(Deserialize)]
struct PublishedWorker {
    id: String,
    has_assets: bool,
}

#[derive(Debug)]
struct ApiFailure {
    message: String,
    retryable: bool,
    retry_after: Option<Duration>,
}

impl std::fmt::Display for ApiFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ApiFailure {}

impl ApiFailure {
    fn new(message: impl Into<String>, retryable: bool) -> Self {
        Self {
            message: message.into(),
            retryable,
            retry_after: None,
        }
    }
}

impl CloudflareClient {
    pub fn new(token: &str) -> Result<Self> {
        validate_token(token)?;
        Ok(Self {
            client: Client::builder()
                .user_agent(concat!("Regressify-Desktop/", env!("CARGO_PKG_VERSION")))
                .redirect(reqwest::redirect::Policy::none())
                .https_only(!cfg!(test))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(180))
                .build()?,
            endpoint: Url::parse("https://api.cloudflare.com/client/v4/")?,
            authorization: bearer(token)?,
            backoff: Duration::from_secs(1),
            read_timeout: Duration::from_secs(30),
            // Leave time before Cloudflare's one-hour upload/completion tokens expire.
            deadline: Duration::from_secs(55 * 60),
            verify_interval: Duration::from_secs(2),
            verify_attempts: 30,
            #[cfg(test)]
            verification_url: None,
        })
    }

    async fn request<Value: DeserializeOwned>(
        &self,
        build: impl Fn() -> Result<RequestBuilder>,
        retry: bool,
    ) -> Result<Value> {
        self.request_protected(build, retry, &[]).await
    }

    async fn request_protected<Value: DeserializeOwned>(
        &self,
        build: impl Fn() -> Result<RequestBuilder>,
        retry: bool,
        secrets: &[&str],
    ) -> Result<Value> {
        let attempts = if retry { 6 } else { 1 };
        for attempt in 0..attempts {
            let request = build()?
                .build()
                .map_err(|_| anyhow!("Cannot construct Cloudflare request"))?;
            let authorization = request
                .headers()
                .get(AUTHORIZATION)
                .cloned()
                .unwrap_or_else(|| self.authorization.clone());
            let result = match self.client.execute(request).await {
                Ok(response) => {
                    api_response(response, &[&self.authorization, &authorization], secrets).await
                }
                Err(_) => Err(ApiFailure::new(
                    "Cloudflare request was interrupted or timed out",
                    true,
                )),
            };
            match result {
                Ok(value) => return Ok(value),
                Err(error) if error.retryable && attempt + 1 < attempts => {
                    let delay = error.retry_after.unwrap_or(self.backoff * (1 << attempt));
                    super::phase(&format!(
                        "Cloudflare retry {}/{} in {:.1}s",
                        attempt + 2,
                        attempts,
                        delay.as_secs_f64()
                    ));
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
        bail!("Cloudflare request retry budget exhausted")
    }

    async fn get<Value: DeserializeOwned>(&self, path: &str) -> Result<Value> {
        let url = self.endpoint.join(path)?;
        self.request(
            || {
                Ok(self
                    .client
                    .get(url.clone())
                    .header(AUTHORIZATION, self.authorization.clone())
                    .timeout(self.read_timeout))
            },
            true,
        )
        .await
    }

    pub async fn deploy(&self, root: &Path, target: &CloudflareTarget) -> Result<String> {
        target.validate()?;
        tokio::time::timeout(self.deadline, self.deploy_inner(root, target))
            .await
            .map_err(|_| anyhow!("Cloudflare deployment exceeded its deadline; check the Worker's deployment history before retrying"))?
    }

    async fn deploy_inner(&self, root: &Path, target: &CloudflareTarget) -> Result<String> {
        super::validate_static_configuration(root, super::DeploymentProvider::CloudflareWorkers)?;
        let deadline = Instant::now() + self.deadline;
        let account = format!("accounts/{}", target.account_id);
        let worker = format!("{account}/workers/scripts/{}", target.worker_name);
        super::phase("Validating Cloudflare deployment target");
        let _: serde::de::IgnoredAny = self.get(&format!("{worker}/settings")).await?;
        let subdomain: AccountSubdomain = self.get(&format!("{account}/workers/subdomain")).await?;
        let route: WorkerRoute = self.get(&format!("{worker}/subdomain")).await?;
        ensure!(
            route.enabled,
            "Enable the selected Worker's workers.dev route in Cloudflare before deploying"
        );
        let public_url = worker_url(&target.worker_name, &subdomain.subdomain)?;
        #[cfg(test)]
        let public_url = self.verification_url.clone().unwrap_or(public_url);
        super::phase("Hashing selected report files for Cloudflare");
        let directory = root.to_path_buf();
        let (manifest, headers, redirects) = tokio::task::spawn_blocking(move || {
            let manifest = Manifest::create(&directory, deadline)?;
            let headers = hosting_policy(&manifest.root, "_headers")?;
            let redirects = cloudflare_redirects(&hosting_policy(&manifest.root, "_redirects")?);
            Ok::<_, anyhow::Error>((manifest, headers, redirects))
        })
        .await??;
        let body = serde_json::to_vec(&serde_json::json!({"manifest": &manifest.files}))?;
        ensure!(
            body.len() <= MAX_MANIFEST_BYTES,
            "Cloudflare asset manifest exceeds 32 MiB"
        );
        super::phase(&format!(
            "Creating Cloudflare upload session for {} files ({:.2} MiB)",
            manifest.files.len(),
            manifest.bytes as f64 / (1024.0 * 1024.0)
        ));
        let session_url = self
            .endpoint
            .join(&format!("{worker}/assets-upload-session"))?;
        let session: UploadSession = self
            .request(
                || {
                    Ok(self
                        .client
                        .post(session_url.clone())
                        .header(AUTHORIZATION, self.authorization.clone())
                        .header("Content-Type", "application/json")
                        .timeout(self.read_timeout)
                        .body(body.clone()))
                },
                false,
            )
            .await?;
        manifest.validate_buckets(&session.buckets)?;
        let upload_token = zeroize::Zeroizing::new(session.jwt);
        let upload_authorization = bearer(&upload_token)?;
        let manifest = Arc::new(manifest);
        let completion = if session.buckets.is_empty() {
            super::phase("Cloudflare already has all report assets; no uploads required");
            upload_token
        } else {
            self.upload_assets(
                manifest.clone(),
                session.buckets,
                &account,
                upload_authorization,
            )
            .await?
        };
        let metadata = zeroize::Zeroizing::new(serde_json::to_string(&serde_json::json!({
            "compatibility_date": "2026-09-01",
            "assets": {
                "jwt": completion.as_str(),
                "config": {
                    "_headers": headers,
                    "_redirects": redirects,
                    "html_handling": "auto-trailing-slash",
                    "not_found_handling": "none"
                }
            }
        }))?);
        let publish_url = self.endpoint.join(&worker)?;
        super::phase("Publishing Cloudflare report assets");
        // Publication is not retried: a lost response must not overwrite a later deployment.
        let published: PublishedWorker = self.request_protected(|| Ok(self.client.put(publish_url.clone())
            .header(AUTHORIZATION, self.authorization.clone())
            .timeout(self.read_timeout)
            .multipart(multipart::Form::new().part("metadata", multipart::Part::text(metadata.to_string()).mime_str("application/json")?))), false, &[completion.as_str()])
            .await.context("Cloudflare publication was not confirmed; check the Worker's deployment history before retrying")?;
        ensure!(
            published.id == target.worker_name && published.has_assets,
            "Cloudflare returned a different Worker or no deployed assets"
        );
        super::phase("Verifying published Cloudflare report");
        self.verify_publication(&public_url, manifest).await?;
        super::phase(&format!("Deployed: {public_url}"));
        Ok(public_url.to_string())
    }

    async fn upload_assets(
        &self,
        manifest: Arc<Manifest>,
        buckets: Vec<Vec<String>>,
        account: &str,
        authorization: HeaderValue,
    ) -> Result<zeroize::Zeroizing<String>> {
        let url = self
            .endpoint
            .join(&format!("{account}/workers/assets/upload?base64=true"))?;
        let total = buckets.len();
        let results = stream::iter(buckets.into_iter().enumerate())
            .map(|(index, hashes)| {
                let manifest = manifest.clone();
                let url = url.clone();
                let authorization = authorization.clone();
                async move {
                    let assets = tokio::task::spawn_blocking(move || {
                        hashes
                            .into_iter()
                            .map(|hash| {
                                let (path, bytes) = manifest.read_asset(&hash)?;
                                Ok::<_, anyhow::Error>((
                                    hash,
                                    STANDARD.encode(bytes),
                                    mime_type(&path),
                                ))
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .await??;
                    // Retries resend these bounded, verified bytes, never a changed file on disk.
                    let result: UploadResult = self
                        .request(
                            || {
                                let mut form = multipart::Form::new();
                                for (hash, content, mime) in &assets {
                                    form = form.part(
                                        hash.clone(),
                                        multipart::Part::bytes(content.as_bytes().to_vec())
                                            .file_name(hash.clone())
                                            .mime_str(mime)?,
                                    );
                                }
                                Ok(self
                                    .client
                                    .post(url.clone())
                                    .header(AUTHORIZATION, authorization.clone())
                                    .timeout(self.deadline)
                                    .multipart(form))
                            },
                            true,
                        )
                        .await?;
                    super::phase(&format!(
                        "Cloudflare accepted asset batch {}/{}",
                        index + 1,
                        total
                    ));
                    Ok::<_, anyhow::Error>(result.jwt.map(zeroize::Zeroizing::new))
                }
            })
            .buffer_unordered(UPLOAD_CONCURRENCY)
            .try_collect::<Vec<_>>()
            .await?;
        let token = results
            .into_iter()
            .flatten()
            .last()
            .context("Cloudflare did not return an asset completion token")?;
        bearer(&token)?;
        Ok(token)
    }

    async fn verify_publication(&self, base: &Url, manifest: Arc<Manifest>) -> Result<()> {
        let samples = tokio::task::spawn_blocking(move || {
            let mut paths = vec!["/index.html".to_string()];
            if let Some(path) = manifest.files.keys().find(|path| !path.ends_with(".html")) {
                paths.push(path.clone());
            }
            paths
                .into_iter()
                .map(|path| {
                    let (_, bytes) = manifest.read_asset(&manifest.files[&path].hash)?;
                    Ok::<_, anyhow::Error>((path, bytes))
                })
                .collect::<Result<Vec<_>>>()
        })
        .await??;
        for attempt in 0..self.verify_attempts {
            let mut verified = true;
            for (path, expected) in &samples {
                let mut url = base.clone();
                url.set_path(if path == "/index.html" { "/" } else { path });
                url.query_pairs_mut()
                    .append_pair("regressify-verify", &uuid::Uuid::new_v4().to_string());
                let response = self.client.get(url).timeout(self.read_timeout).send().await;
                match response {
                    Ok(response) if response.status().is_success() => {
                        let content_type = response
                            .headers()
                            .get("Content-Type")
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or_default();
                        if content_type.split(';').next() != mime_type(path).split(';').next()
                            || !response_bytes(response, MAX_FILE_BYTES as usize)
                                .await
                                .is_ok_and(|bytes| bytes.as_slice() == expected.as_slice())
                        {
                            verified = false;
                        }
                    }
                    _ => verified = false,
                }
            }
            if verified {
                return Ok(());
            }
            if attempt + 1 < self.verify_attempts {
                tokio::time::sleep(self.verify_interval).await;
            }
        }
        bail!(
            "Cloudflare accepted the deployment, but the public report could not be verified; check its workers.dev route, Access policy, and deployment history before retrying"
        )
    }
}

fn bearer(token: &str) -> Result<HeaderValue> {
    ensure!(
        !token.is_empty()
            && token.len() <= 32 * 1024
            && token.bytes().all(|byte| byte.is_ascii_graphic()),
        "Cloudflare returned an invalid authorization token"
    );
    let value = zeroize::Zeroizing::new(format!("Bearer {token}"));
    let mut header = HeaderValue::from_str(&value)
        .map_err(|_| anyhow!("Invalid Cloudflare authorization token"))?;
    header.set_sensitive(true);
    Ok(header)
}

fn worker_url(worker: &str, subdomain: &str) -> Result<Url> {
    validate_dns_label(worker, "Worker name")?;
    validate_dns_label(subdomain, "account subdomain")?;
    Url::parse(&format!("https://{worker}.{subdomain}.workers.dev/"))
        .context("Invalid Cloudflare public URL")
}

fn hosting_policy(root: &Path, name: &str) -> Result<String> {
    let Some(path) = super::artifact_source(root, Path::new(name), true)? else {
        return Ok(String::new());
    };
    let bytes = super::bounded_read(
        std::fs::File::open(path)?,
        256 * 1024,
        "Cloudflare hosting policy",
    )?;
    String::from_utf8(bytes).context("Cloudflare hosting policies must be UTF-8")
}

fn cloudflare_redirects(policy: &str) -> String {
    policy
        .lines()
        .map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            // Workers rejects index.html proxies as redirect loops; use its canonical root instead.
            match fields.as_slice() {
                [source, "/index.html", "200"] => format!("{source} / 200"),
                _ => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn mime_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "xml" | "xsl" => "application/xml; charset=utf-8",
        "txt" | "map" | "sh" | "ps1" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn response_bytes(
    response: Response,
    limit: usize,
) -> std::result::Result<Vec<u8>, ApiFailure> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(ApiFailure::new(
            "Cloudflare response exceeds its size limit",
            false,
        ));
    }
    let mut bytes = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|_| {
            ApiFailure::new(
                "Cloudflare response body was interrupted or timed out",
                true,
            )
        })?;
        if bytes.len() + chunk.len() > limit {
            return Err(ApiFailure::new(
                "Cloudflare response exceeds its size limit",
                false,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn api_response<Value: DeserializeOwned>(
    response: Response,
    authorizations: &[&HeaderValue],
    secrets: &[&str],
) -> std::result::Result<Value, ApiFailure> {
    let status = response.status();
    let retry_after = response
        .headers()
        .get("Retry-After")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| Duration::from_secs(seconds.min(60)));
    let ray = response
        .headers()
        .get("cf-ray")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        .unwrap_or_default()
        .to_string();
    let bytes = zeroize::Zeroizing::new(response_bytes(response, MAX_RESPONSE_BYTES).await?);
    let envelope: ApiEnvelope<Value> = serde_json::from_slice(&bytes).map_err(|_| {
        ApiFailure::new(
            format!(
                "Cloudflare returned an invalid API response (HTTP {})",
                status.as_u16()
            ),
            status.is_server_error() || status.as_u16() == 429,
        )
    })?;
    if status.is_success() && envelope.success {
        return envelope
            .result
            .ok_or_else(|| ApiFailure::new("Cloudflare returned no API result", false));
    }
    let errors = envelope.errors.unwrap_or_default();
    let codes = errors
        .iter()
        .take(8)
        .map(|error| error.code.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let mut detail = errors
        .iter()
        .take(8)
        .map(|error| error.message.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    // Redact every credential used by this request, including the completion JWT in publication metadata.
    for secret in secrets.iter().copied().filter(|secret| !secret.is_empty()) {
        detail = detail.replace(secret, "[redacted]");
    }
    for authorization in authorizations {
        if let Some(token) = authorization
            .to_str()
            .ok()
            .and_then(|value| value.strip_prefix("Bearer "))
        {
            detail = detail.replace(token, "[redacted]");
        }
    }
    if let Some(authorization) = authorizations.first() {
        detail = super::diagnostics::sanitize(&detail, authorization);
    }
    let hint = match status.as_u16() {
        401 | 403 => "Check the API token's Workers Scripts Write permission and selected account",
        404 => "Check the Account ID and existing Worker name",
        413 => "Check the 25 MiB per-file and account file-count limits",
        429 => "Cloudflare rate limit reached",
        _ => "Check the Worker's deployment history and Cloudflare account limits",
    };
    Err(ApiFailure {
        message: format!(
            "Cloudflare request failed (HTTP {}; codes: {codes}; request ID: {ray}): {detail}. {hint}",
            status.as_u16()
        ),
        retryable: status.is_server_error() || matches!(status.as_u16(), 408 | 429),
        retry_after,
    })
}

#[derive(Clone, Debug, Serialize)]
struct Asset {
    hash: String,
    size: u64,
}

struct Manifest {
    root: PathBuf,
    files: BTreeMap<String, Asset>,
    contents: BTreeMap<String, String>,
    bytes: u64,
}

impl Manifest {
    fn create(root: &Path, deadline: Instant) -> Result<Self> {
        Self::create_bounded(root, deadline, MAX_FILES, MAX_CONTENT_BYTES, MAX_FILE_BYTES)
    }

    fn create_bounded(
        root: &Path,
        deadline: Instant,
        file_limit: usize,
        byte_limit: u64,
        file_byte_limit: u64,
    ) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(root)?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Cloudflare staging root must be a regular directory"
        );
        let mut manifest = Self {
            root: root.canonicalize()?,
            files: BTreeMap::new(),
            contents: BTreeMap::new(),
            bytes: 0,
        };
        let mut last_update = Instant::now();
        for entry in walkdir::WalkDir::new(&manifest.root)
            .follow_links(false)
            .sort_by_file_name()
        {
            ensure!(
                Instant::now() < deadline,
                "Cloudflare hashing exceeded the deployment deadline"
            );
            let entry = entry?;
            ensure!(
                !entry.file_type().is_symlink(),
                "Cloudflare deployment cannot contain symlinks"
            );
            if entry.file_type().is_dir() {
                continue;
            }
            ensure!(
                entry.file_type().is_file(),
                "Cloudflare deployment requires regular files"
            );
            let relative = entry.path().strip_prefix(&manifest.root)?;
            let path = relative
                .components()
                .map(|component| {
                    component
                        .as_os_str()
                        .to_str()
                        .context("Deployment paths must be UTF-8")
                })
                .collect::<Result<Vec<_>>>()?
                .join("/");
            ensure!(
                !path.contains(['#', '?', '\\']) && !path.chars().any(char::is_control),
                "Cloudflare deployment path contains unsupported characters"
            );
            if matches!(
                path.as_str(),
                "_headers" | "_redirects" | "staticwebapp.config.json" | ".gitignore"
            ) {
                continue;
            }
            ensure!(
                manifest.files.len() < file_limit,
                "Cloudflare deployment exceeds the {file_limit}-file client limit"
            );
            let source = super::artifact_source(&manifest.root, relative, false)?
                .context("Cloudflare deployment file is unavailable")?;
            let size = std::fs::metadata(&source)?.len();
            ensure!(
                size <= file_byte_limit,
                "Cloudflare asset '{path}' exceeds the {} MiB per-file limit",
                file_byte_limit as f64 / (1024.0 * 1024.0)
            );
            manifest.bytes = manifest
                .bytes
                .checked_add(size)
                .context("Cloudflare content size overflow")?;
            ensure!(
                manifest.bytes <= byte_limit,
                "Cloudflare deployment exceeds its {} MiB client content limit",
                byte_limit as f64 / (1024.0 * 1024.0)
            );
            let bytes = read_file(&source, size)?;
            let hash = asset_hash(&bytes, &path);
            manifest
                .contents
                .entry(hash.clone())
                .or_insert_with(|| path.clone());
            manifest
                .files
                .insert(format!("/{path}"), Asset { hash, size });
            if manifest.files.len() == 1
                || manifest.files.len().is_multiple_of(50)
                || last_update.elapsed().as_secs() >= 1
            {
                super::phase(&format!(
                    "Hashed {} Cloudflare report files ({:.2} MiB)",
                    manifest.files.len(),
                    manifest.bytes as f64 / (1024.0 * 1024.0)
                ));
                last_update = Instant::now();
            }
        }
        ensure!(
            manifest.files.contains_key("/index.html"),
            "Deployment requires index.html"
        );
        Ok(manifest)
    }

    fn read_asset(&self, hash: &str) -> Result<(String, Vec<u8>)> {
        let path = self
            .contents
            .get(hash)
            .context("Cloudflare requested content outside the deployment manifest")?;
        let asset = &self.files[&format!("/{path}")];
        let source = super::artifact_source(&self.root, Path::new(path), false)?
            .context("Cloudflare deployment file is unavailable")?;
        let bytes = read_file(&source, asset.size)?;
        ensure!(
            asset_hash(&bytes, path) == hash,
            "Cloudflare deployment file changed after hashing"
        );
        Ok((path.clone(), bytes))
    }

    fn validate_buckets(&self, buckets: &[Vec<String>]) -> Result<()> {
        let mut seen = BTreeSet::new();
        for bucket in buckets {
            ensure!(
                !bucket.is_empty(),
                "Cloudflare returned an empty upload bucket"
            );
            let mut bytes = 0_u64;
            for hash in bucket {
                ensure!(
                    hash.len() == 32 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
                    "Cloudflare returned an invalid asset hash"
                );
                let path = self
                    .contents
                    .get(hash)
                    .context("Cloudflare requested content outside the deployment manifest")?;
                ensure!(
                    seen.insert(hash),
                    "Cloudflare requested duplicate upload content"
                );
                bytes = bytes
                    .checked_add(self.files[&format!("/{path}")].size)
                    .context("Cloudflare bucket size overflow")?;
                ensure!(
                    bytes <= MAX_BUCKET_BYTES,
                    "Cloudflare upload bucket exceeds its memory limit"
                );
            }
        }
        Ok(())
    }
}

fn read_file(path: &Path, expected: u64) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "Cloudflare deployment requires regular files"
    );
    let mut bytes = Vec::new();
    file.take(expected + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == expected,
        "Cloudflare deployment file changed after hashing"
    );
    Ok(bytes)
}

fn asset_hash(bytes: &[u8], path: &str) -> String {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    // Cloudflare's upload identity includes the extension because identical bytes can have different MIME types.
    let mut digest = Sha256::new();
    digest.update(STANDARD.encode(bytes));
    digest.update(extension);
    format!("{:x}", digest.finalize())[..32].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    const TOKEN: &str = "synthetic-cloudflare-token";

    fn client(origin: &str) -> CloudflareClient {
        let mut client = CloudflareClient::new(TOKEN).unwrap();
        client.endpoint = Url::parse(&format!("{origin}/api/")).unwrap();
        client.backoff = Duration::from_millis(1);
        client.verify_interval = Duration::from_millis(1);
        client.verify_attempts = 2;
        client.verification_url = Some(Url::parse(&format!("{origin}/")).unwrap());
        client
    }

    #[tokio::test]
    async fn cached_deployment_publishes_metadata_without_uploading_assets() -> Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("index.html"), "report")?;
        std::fs::write(root.path().join("_headers"), "/*\n  X-Robots-Tag: noindex")?;
        std::fs::write(root.path().join("_redirects"), "/suites/* /index.html 200")?;
        let methods = Arc::new(Mutex::new(Vec::new()));
        let captured = methods.clone();
        let site = crate::test::browser_tests::Site::new(move |mut request| {
            let path = request.url().to_owned();
            let method = request.method().as_str().to_owned();
            captured
                .lock()
                .unwrap()
                .push((method.clone(), path.clone()));
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            let value = if path.ends_with("/settings") {
                json!({})
            } else if path.ends_with("/workers/subdomain") {
                json!({"subdomain":"account"})
            } else if path.ends_with("/subdomain") {
                json!({"enabled":true})
            } else if path.ends_with("/assets-upload-session") {
                let manifest: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(manifest["manifest"].as_object().unwrap().len(), 1);
                assert_eq!(manifest["manifest"]["/index.html"]["size"], 6);
                json!({"jwt":"cached-completion-token","buckets":[]})
            } else if method == "PUT" {
                assert!(body.contains("cached-completion-token"));
                assert!(body.contains("X-Robots-Tag: noindex"));
                assert!(body.contains("/suites/* / 200"));
                assert!(body.contains("\"not_found_handling\":\"none\""));
                assert!(!body.contains("main_module"));
                json!({"id":"report-worker","has_assets":true})
            } else {
                assert!(path.starts_with("/?regressify-verify="));
                assert!(
                    !request
                        .headers()
                        .iter()
                        .any(|header| header.field.equiv("Authorization"))
                );
                let _ = request.respond(
                    tiny_http::Response::from_string("report").with_header(
                        tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8")
                            .unwrap(),
                    ),
                );
                return;
            };
            let _ = request.respond(tiny_http::Response::from_string(
                json!({"success":true,"result":value}).to_string(),
            ));
        })?;
        assert_eq!(
            client(&site.origin).deploy(root.path(), &target()).await?,
            format!("{}/", site.origin)
        );
        let methods = methods.lock().unwrap();
        assert_eq!(
            methods
                .iter()
                .filter(|(method, _)| method == "POST")
                .count(),
            1
        );
        assert_eq!(
            methods.iter().filter(|(method, _)| method == "PUT").count(),
            1
        );
        assert_eq!(methods.len(), 6);
        Ok(())
    }

    #[tokio::test]
    async fn azure_access_policies_are_rejected_before_api_requests() -> Result<()> {
        let requests = Arc::new(AtomicUsize::new(0));
        let captured = requests.clone();
        let site = crate::test::browser_tests::Site::new(move |request| {
            captured.fetch_add(1, Ordering::SeqCst);
            let _ = request.respond(
                tiny_http::Response::from_string(
                    json!({"success":false,"errors":[{"code":10000}]}).to_string(),
                )
                .with_status_code(403),
            );
        })?;
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("index.html"), "report")?;
        for policy in [
            json!({"auth":{}}),
            json!({"networking":{}}),
            json!({"routes":[{"route":"/*","allowedRoles":["authenticated"]}]}),
        ] {
            std::fs::write(
                root.path().join("staticwebapp.config.json"),
                serde_json::to_vec(&policy)?,
            )?;
            let error = client(&site.origin)
                .deploy(root.path(), &target())
                .await
                .err()
                .context("Expected restriction rejection")?;
            assert!(
                error.to_string().contains("Azure access restrictions"),
                "{error}"
            );
            assert_eq!(requests.load(Ordering::SeqCst), 0);
        }
        Ok(())
    }

    #[tokio::test]
    async fn invalid_routes_and_unknown_hashes_fail_before_publication() -> Result<()> {
        for mode in ["disabled", "unsafe-subdomain", "unknown-hash"] {
            let root = tempfile::tempdir()?;
            std::fs::write(root.path().join("index.html"), "report")?;
            let writes = Arc::new(AtomicUsize::new(0));
            let captured = writes.clone();
            let site = crate::test::browser_tests::Site::new(move |request| {
                let path = request.url();
                if request.method().as_str() == "PUT" || path.contains("assets/upload?") {
                    captured.fetch_add(1, Ordering::SeqCst);
                }
                let value = if path.ends_with("/settings") {
                    json!({})
                } else if path.ends_with("/workers/subdomain") {
                    json!({"subdomain":if mode == "unsafe-subdomain" { "attacker.invalid/" } else { "account" }})
                } else if path.ends_with("/subdomain") {
                    json!({"enabled":mode != "disabled"})
                } else {
                    json!({"jwt":"session-token","buckets":[["0".repeat(32)]]})
                };
                let _ = request.respond(tiny_http::Response::from_string(
                    json!({"success":true,"result":value}).to_string(),
                ));
            })?;
            assert!(
                client(&site.origin)
                    .deploy(root.path(), &target())
                    .await
                    .is_err()
            );
            assert_eq!(writes.load(Ordering::SeqCst), 0);
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore = "publishes CLOUDFLARE_REPORT_ROOT twice; requires API token, Account ID, Worker name, suite IDs and expected URL; replaces Worker assets"]
    async fn live_publish_cloudflare_reports() -> Result<()> {
        let token = zeroize::Zeroizing::new(
            std::env::var("CLOUDFLARE_API_TOKEN").context("Set CLOUDFLARE_API_TOKEN")?,
        );
        let target = CloudflareTarget {
            account_id: std::env::var("CLOUDFLARE_ACCOUNT_ID")
                .context("Set CLOUDFLARE_ACCOUNT_ID")?,
            worker_name: std::env::var("CLOUDFLARE_WORKER_NAME")
                .context("Set CLOUDFLARE_WORKER_NAME")?,
        };
        target.validate()?;
        let expected_url = std::env::var("CLOUDFLARE_EXPECTED_URL")
            .context("Set CLOUDFLARE_EXPECTED_URL to authorize the destination")?;
        let output = PathBuf::from(
            std::env::var_os("CLOUDFLARE_REPORT_ROOT").context("Set CLOUDFLARE_REPORT_ROOT")?,
        );
        let suites = std::env::var("CLOUDFLARE_SUITE_IDS")
            .context("Set CLOUDFLARE_SUITE_IDS to comma-separated selected suite IDs")?
            .split(',')
            .map(|value| value.trim().to_string())
            .collect::<Vec<_>>();
        for suite in &suites {
            super::super::validate_suite_id(suite)?;
        }
        let client = CloudflareClient::new(&token)?;
        let subdomain: AccountSubdomain = client
            .get(&format!("accounts/{}/workers/subdomain", target.account_id))
            .await?;
        ensure!(
            worker_url(&target.worker_name, &subdomain.subdomain)?.as_str() == expected_url,
            "Cloudflare target does not match the explicitly authorized public URL"
        );
        let workspace =
            crate::config::load_workspace_config(Path::new(env!("CARGO_MANIFEST_DIR")))?;
        crate::snapshot::generate_site(
            &output,
            &suites,
            &crate::cli::args::SnapshotArgs {
                test_suite: suites.clone(),
                title: None,
                heading: None,
                subheading: None,
            },
            &workspace,
        )?;
        let directory = tempfile::tempdir()?;
        let staging = directory.path().join("report");
        super::super::stage_reports(&output, &suites, &staging)?;
        let manifest = Manifest::create(&staging, deadline())?;
        let mut published = String::new();
        for attempt in 1..=2 {
            published = client.deploy(&staging, &target).await?;
            ensure!(
                published == expected_url,
                "Published URL differs from the authorized destination"
            );
            println!("Live Cloudflare deployment {attempt}/2 verified: {published}");
        }
        let base = Url::parse(&published)?;
        stream::iter(manifest.files.iter())
            .map(|(path, asset)| {
                let client = &client.client;
                let mut url = base.clone();
                async move {
                    url.set_path(if path == "/index.html" { "/" } else { path });
                    let response = client
                        .get(url)
                        .timeout(Duration::from_secs(120))
                        .send()
                        .await
                        .map_err(|_| {
                            anyhow!("Cannot retrieve published Cloudflare report asset")
                        })?;
                    ensure!(
                        response.status().is_success(),
                        "A published Cloudflare asset is unavailable"
                    );
                    ensure!(
                        response
                            .headers()
                            .get("X-Content-Type-Options")
                            .and_then(|value| value.to_str().ok())
                            == Some("nosniff"),
                        "Published security headers are missing"
                    );
                    if path.ends_with(".png")
                        || path.ends_with(".json")
                        || path.ends_with(".js")
                        || path.ends_with(".css")
                    {
                        ensure!(
                            response
                                .headers()
                                .get("Cache-Control")
                                .and_then(|value| value.to_str().ok())
                                .is_some_and(|value| value.contains("immutable")),
                            "Published asset cache policy is missing"
                        );
                    }
                    let bytes = response_bytes(response, MAX_FILE_BYTES as usize).await?;
                    ensure!(
                        bytes.len() as u64 == asset.size && asset_hash(&bytes, path) == asset.hash,
                        "Published asset differs from the staged report"
                    );
                    Ok::<_, anyhow::Error>(())
                }
            })
            .buffer_unordered(4)
            .try_collect::<Vec<_>>()
            .await?;
        for suite in &suites {
            let mut url = base.clone();
            url.path_segments_mut()
                .map_err(|_| anyhow!("Invalid public URL"))?
                .extend(["suites", suite]);
            let response = client
                .client
                .get(url)
                .send()
                .await
                .map_err(|_| anyhow!("Cannot retrieve suite deep link"))?;
            ensure!(
                response.status().is_success(),
                "Suite deep link is unavailable"
            );
            let bytes = response_bytes(response, MAX_FILE_BYTES as usize).await?;
            ensure!(
                asset_hash(&bytes, "index.html") == manifest.files["/index.html"].hash,
                "Suite deep link did not serve the report application"
            );
        }
        for path in [
            "_snapshot/data/missing.json",
            "_snapshot/images/missing.png",
            "_headers",
            "_redirects",
        ] {
            let response = client
                .client
                .get(base.join(path)?)
                .send()
                .await
                .map_err(|_| anyhow!("Cannot verify missing-asset behavior"))?;
            ensure!(
                response.status() == reqwest::StatusCode::NOT_FOUND,
                "Missing assets or hosting policies must return 404"
            );
        }
        println!(
            "Verified {} public report assets, suite deep links, security/cache headers, and missing-file 404s",
            manifest.files.len()
        );
        Ok(())
    }

    #[tokio::test]
    async fn upload_batches_use_session_authorization_and_verified_base64() -> Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("index.html"), "report")?;
        let manifest = Arc::new(Manifest::create(root.path(), deadline())?);
        let hash = manifest.files["/index.html"].hash.clone();
        let expected_hash = hash.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let captured = calls.clone();
        let site = crate::test::browser_tests::Site::new(move |mut request| {
            assert_eq!(
                request.url(),
                "/api/accounts/test/workers/assets/upload?base64=true"
            );
            assert_eq!(request.method().as_str(), "POST");
            assert!(
                request
                    .headers()
                    .iter()
                    .any(|header| header.field.equiv("Authorization")
                        && header.value.as_str() == "Bearer upload-session-token")
            );
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            assert!(body.contains(&expected_hash));
            assert!(body.contains("cmVwb3J0"));
            assert!(body.contains("text/html; charset=utf-8"));
            let attempt = captured.fetch_add(1, Ordering::SeqCst);
            let response = if attempt == 0 {
                tiny_http::Response::from_string(
                    json!({"success":false,"errors":[{"code":10000,"message":TOKEN}]}).to_string(),
                )
                .with_status_code(503)
            } else {
                tiny_http::Response::from_string(
                    json!({"success":true,"result":{"jwt":"completion-token"}}).to_string(),
                )
                .with_status_code(201)
            };
            let _ = request.respond(response);
        })?;
        let token = client(&site.origin)
            .upload_assets(
                manifest,
                vec![vec![hash]],
                "accounts/test",
                bearer("upload-session-token")?,
            )
            .await?;
        assert_eq!(token.as_str(), "completion-token");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        Ok(())
    }

    #[tokio::test]
    async fn api_retries_are_bounded_and_failures_do_not_echo_secrets() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let captured = calls.clone();
        let site = crate::test::browser_tests::Site::new(move |request| {
            captured.fetch_add(1, Ordering::SeqCst);
            let _ = request.respond(tiny_http::Response::from_string(json!({"success":false,"errors":[{"code":10000,"message":format!("{TOKEN} upload-session-token completion-token https://private.invalid/?secret=hidden")}]}).to_string()).with_status_code(503));
        })?;
        let client = client(&site.origin);
        let url = client.endpoint.join("test")?;
        let error = client
            .request_protected::<WorkerRoute>(
                || {
                    Ok(client
                        .client
                        .get(url.clone())
                        .header(AUTHORIZATION, client.authorization.clone()))
                },
                true,
                &["upload-session-token", "completion-token"],
            )
            .await
            .err()
            .context("Expected failure")?;
        let message = format!("{error:#}");
        assert!(message.contains("10000"));
        for secret in [TOKEN, "upload-session-token", "completion-token", "hidden"] {
            assert!(!message.contains(secret));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 6);
        let url = client.endpoint.join("publish")?;
        assert!(
            client
                .request::<WorkerRoute>(|| Ok(client.client.put(url.clone())), false)
                .await
                .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 7);
        Ok(())
    }

    #[tokio::test]
    async fn permanent_failures_and_oversized_bodies_are_not_retried() -> Result<()> {
        for mode in ["authentication", "redirect", "malformed", "oversized"] {
            let requests = Arc::new(AtomicUsize::new(0));
            let captured = requests.clone();
            let site = crate::test::browser_tests::Site::new(move |request| {
                captured.fetch_add(1, Ordering::SeqCst);
                let (status, body) = match mode {
                    "authentication" => (
                        403,
                        json!({"success":false,"errors":[{"code":10000,"message":TOKEN}]})
                            .to_string(),
                    ),
                    "redirect" => (302, String::new()),
                    "malformed" => (200, TOKEN.to_string()),
                    _ => (200, "x".repeat(MAX_RESPONSE_BYTES + 1)),
                };
                let _ = request.respond(
                    tiny_http::Response::from_string(body)
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_bytes("Location", "https://untrusted.invalid/")
                                .unwrap(),
                        ),
                );
            })?;
            let error = client(&site.origin)
                .get::<WorkerRoute>("test")
                .await
                .err()
                .context("Expected failure")?;
            assert!(!format!("{error:#}").contains(TOKEN));
            assert_eq!(requests.load(Ordering::SeqCst), 1);
        }
        Ok(())
    }

    #[tokio::test]
    async fn upload_credentials_are_redacted_before_diagnostic_truncation() -> Result<()> {
        let secret = "sensitive-upload-secret".repeat(10);
        let echoed = secret.clone();
        let site = crate::test::browser_tests::Site::new(move |request| {
            let body = json!({"success":false,"errors":[{"code":10000,"message":format!("{}{echoed}", "x".repeat(2020))}]}).to_string();
            let _ = request.respond(tiny_http::Response::from_string(body).with_status_code(403));
        })?;
        let client = client(&site.origin);
        let url = client.endpoint.join("test")?;
        let authorization = bearer(&secret)?;
        let error = client
            .request::<WorkerRoute>(
                || {
                    Ok(client
                        .client
                        .get(url.clone())
                        .header(AUTHORIZATION, authorization.clone()))
                },
                false,
            )
            .await
            .err()
            .context("Expected failure")?;
        assert!(!error.to_string().contains("sensitive-upload"));
        Ok(())
    }

    #[tokio::test]
    async fn deployment_deadline_stops_waiting_for_provider_headers() -> Result<()> {
        let site = crate::test::browser_tests::Site::new(move |request| {
            std::thread::sleep(Duration::from_millis(100));
            let _ = request.respond(tiny_http::Response::empty(500));
        })?;
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("index.html"), "report")?;
        let mut client = client(&site.origin);
        client.deadline = Duration::from_millis(10);
        let error = tokio::time::timeout(
            Duration::from_secs(1),
            client.deploy(root.path(), &target()),
        )
        .await?
        .err()
        .context("Expected deadline")?;
        assert!(error.to_string().contains("exceeded its deadline"));
        Ok(())
    }

    #[tokio::test]
    async fn publication_verifies_bytes_and_never_forwards_credentials() -> Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("index.html"), "report")?;
        std::fs::write(root.path().join("site.css"), "body{}")?;
        let manifest = Arc::new(Manifest::create(root.path(), deadline())?);
        let paths = Arc::new(Mutex::new(Vec::new()));
        let captured = paths.clone();
        let site = crate::test::browser_tests::Site::new(move |request| {
            assert!(
                !request
                    .headers()
                    .iter()
                    .any(|header| header.field.equiv("Authorization"))
            );
            captured.lock().unwrap().push(request.url().to_owned());
            let css = request.url().starts_with("/site.css?");
            let response = tiny_http::Response::from_string(if css { "body{}" } else { "report" })
                .with_header(
                    tiny_http::Header::from_bytes(
                        "Content-Type",
                        if css {
                            "text/css; charset=utf-8"
                        } else {
                            "text/html; charset=utf-8"
                        },
                    )
                    .unwrap(),
                );
            let _ = request.respond(response);
        })?;
        client(&site.origin)
            .verify_publication(&Url::parse(&site.origin)?, manifest)
            .await?;
        assert_eq!(paths.lock().unwrap().len(), 2);
        assert!(worker_url("worker", "bad.example").is_err());
        assert!(worker_url("worker", "user@attacker").is_err());
        assert_eq!(
            worker_url("report", "account")?.as_str(),
            "https://report.account.workers.dev/"
        );
        Ok(())
    }

    fn target() -> CloudflareTarget {
        CloudflareTarget {
            account_id: "0123456789abcdef0123456789abcdef".into(),
            worker_name: "report-worker".into(),
        }
    }

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }

    #[test]
    fn suite_rewrites_use_cloudflare_canonical_html_without_a_catch_all() {
        assert_eq!(
            cloudflare_redirects(include_str!("../../snapshot/data/_redirects")),
            "/suites/* / 200"
        );
        assert_eq!(
            cloudflare_redirects("# comment\n/suites/* /index.html 200\n/old /new 301"),
            "# comment\n/suites/* / 200\n/old /new 301"
        );
        assert_eq!(cloudflare_redirects(""), "");
    }

    #[test]
    fn target_and_token_validation() -> Result<()> {
        target().validate()?;
        for invalid in [
            "",
            "account",
            "../0123456789abcdef0123456789ab",
            "0123456789abcdef0123456789abcdeg",
        ] {
            assert!(
                CloudflareTarget {
                    account_id: invalid.into(),
                    ..target()
                }
                .validate()
                .is_err()
            );
        }
        for invalid in [
            "",
            "../worker",
            "-worker",
            "worker-",
            "worker.name",
            "worker_name",
            "worker name",
            &"a".repeat(64),
        ] {
            assert!(
                CloudflareTarget {
                    worker_name: invalid.into(),
                    ..target()
                }
                .validate()
                .is_err()
            );
        }
        validate_token("synthetic-cloudflare-token")?;
        for invalid in ["", "token with space", "token\n", &"a".repeat(1025)] {
            assert!(validate_token(invalid).is_err());
        }
        Ok(())
    }

    #[test]
    fn manifest_deduplicates_content_and_excludes_hosting_policies() -> Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::create_dir(root.path().join("assets"))?;
        for path in [
            "index.html",
            "assets/copy.html",
            "assets/different.txt",
            "_headers",
            "_redirects",
            "staticwebapp.config.json",
            ".gitignore",
        ] {
            std::fs::write(root.path().join(path), "report")?;
        }
        let manifest = Manifest::create(root.path(), deadline())?;
        assert_eq!(manifest.files.len(), 3);
        assert_eq!(manifest.contents.len(), 2);
        assert_eq!(manifest.bytes, 18);
        assert_eq!(
            manifest.files["/index.html"].hash,
            manifest.files["/assets/copy.html"].hash
        );
        assert_ne!(
            manifest.files["/index.html"].hash,
            manifest.files["/assets/different.txt"].hash
        );
        assert_eq!(manifest.files["/index.html"].hash.len(), 32);
        let json = serde_json::to_value(&manifest.files)?;
        assert_eq!(json["/index.html"]["size"], 6);
        Ok(())
    }

    #[test]
    fn manifest_enforces_limits_and_rejects_changed_content() -> Result<()> {
        let root = tempfile::tempdir()?;
        assert!(Manifest::create(root.path(), deadline()).is_err());
        std::fs::write(root.path().join("index.html"), "report")?;
        assert!(Manifest::create_bounded(root.path(), deadline(), 0, 6, 6).is_err());
        assert!(Manifest::create_bounded(root.path(), deadline(), 1, 5, 6).is_err());
        assert!(Manifest::create_bounded(root.path(), deadline(), 1, 6, 5).is_err());
        assert!(Manifest::create(root.path(), Instant::now()).is_err());
        let manifest = Manifest::create_bounded(root.path(), deadline(), 1, 6, 6)?;
        let hash = &manifest.files["/index.html"].hash;
        assert_eq!(manifest.read_asset(hash)?.1, b"report");
        for content in ["change", "short", "longer report"] {
            std::fs::write(root.path().join("index.html"), content)?;
            assert!(manifest.read_asset(hash).is_err());
        }
        Ok(())
    }

    #[test]
    fn upload_buckets_only_accept_known_distinct_hashes() -> Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("index.html"), "")?;
        let manifest = Manifest::create(root.path(), deadline())?;
        let hash = manifest.files["/index.html"].hash.clone();
        manifest.validate_buckets(&[])?;
        manifest.validate_buckets(&[vec![hash.clone()]])?;
        for buckets in [
            vec![vec![]],
            vec![vec!["../index.html".into()]],
            vec![vec!["0".repeat(32)]],
            vec![vec![hash.clone(), hash.clone()]],
            vec![vec![hash.clone()], vec![hash]],
        ] {
            assert!(manifest.validate_buckets(&buckets).is_err());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn manifest_rejects_symlinks_and_changed_parents() -> Result<()> {
        let root = tempfile::tempdir()?;
        let staging = root.path().join("stage");
        std::fs::create_dir_all(staging.join("assets"))?;
        std::fs::write(staging.join("index.html"), "report")?;
        std::fs::write(staging.join("assets/file.txt"), "image")?;
        let manifest = Manifest::create(&staging, deadline())?;
        std::os::unix::fs::symlink(&staging, root.path().join("link"))?;
        assert!(Manifest::create(&root.path().join("link"), deadline()).is_err());
        std::fs::rename(staging.join("assets"), staging.join("original"))?;
        std::os::unix::fs::symlink(staging.join("original"), staging.join("assets"))?;
        assert!(Manifest::create(&staging, deadline()).is_err());
        assert!(
            manifest
                .read_asset(&manifest.files["/assets/file.txt"].hash)
                .is_err()
        );
        Ok(())
    }
}
