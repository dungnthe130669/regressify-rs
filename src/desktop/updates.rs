use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use reqwest::{StatusCode, Url, blocking::Client, redirect::Policy};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

pub const CURRENT_VERSION: &str = match option_env!("REGRESSIFY_DESKTOP_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
const BASE_URL: &str = "https://drive.tuyen.blog";
const MAX_MANIFEST_SIZE: u64 = 16 * 1024;
const MAX_INSTALLER_SIZE: u64 = 1024 * 1024 * 1024;
const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version([u16; 4]);

impl FromStr for Version {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let mut parts = [0; 4];
        ensure!(value.len() <= 32, "Version is too long");
        for (index, part) in value.split('.').enumerate() {
            ensure!(
                index < 4 && !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()),
                "Version must have one to four numeric components"
            );
            let number: u16 = part.parse().context("Invalid version component")?;
            ensure!(
                number <= 9999,
                "Version components must be between 0 and 9999"
            );
            parts[index] = number;
        }
        Ok(Self(parts))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{}.{}.{}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

pub fn installer_name(version: Version) -> String {
    format!("Regressify-Desktop-Setup-{version}-x64.exe")
}

#[derive(Clone, Debug)]
pub struct UpdateFeed {
    slug: String,
    manifest_url: Url,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    installer: Url,
    sha256: String,
    size: u64,
}

#[derive(Debug, PartialEq)]
pub enum UpdateCheck {
    Current,
    Unpublished,
    Available(Release),
}

#[derive(Deserialize)]
struct ManifestFields {
    version: String,
    installer: String,
    sha256: String,
    size: serde_json::Value,
}

impl UpdateFeed {
    pub fn new(slug: &str) -> Result<Self> {
        ensure!(
            !slug.is_empty()
                && slug.len() <= 64
                && slug.bytes().all(|byte| byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || byte == b'-'
                    || byte == b'_'),
            "Invalid Tuyen Drive app slug"
        );
        Ok(Self {
            slug: slug.into(),
            manifest_url: format!("{BASE_URL}/downloads/{slug}/latest.txt").parse()?,
        })
    }

    pub fn configured() -> Option<Self> {
        if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            return None;
        }
        option_env!("TUYEN_DRIVE_APP").and_then(|slug| Self::new(slug).ok())
    }

    pub fn parse_manifest(&self, text: &str) -> Result<Release> {
        ensure!(
            text.len() as u64 <= MAX_MANIFEST_SIZE,
            "Update manifest is too large"
        );
        let text = text.trim_start_matches('\u{feff}').trim();
        let fields: ManifestFields = if text.starts_with('{') {
            serde_json::from_str(text).context("Invalid JSON update manifest")?
        } else {
            let mut fields = BTreeMap::new();
            for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                let (key, value) = line
                    .split_once('=')
                    .context("Invalid update manifest line")?;
                ensure!(
                    fields.insert(key.trim(), value.trim()).is_none(),
                    "Duplicate update manifest field: {key}"
                );
            }
            serde_json::from_value(serde_json::to_value(fields)?)
                .context("Missing required update manifest fields")?
        };
        let version: Version = fields.version.parse()?;
        let installer: Url = fields.installer.parse().context("Invalid installer URL")?;
        let expected = format!(
            "{BASE_URL}/downloads/{}/{}",
            self.slug,
            installer_name(version)
        );
        ensure!(
            fields.installer == expected && installer.as_str() == expected,
            "Installer must use this app's immutable HTTPS download URL"
        );
        ensure!(
            fields.sha256.len() == 64 && fields.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Installer SHA-256 must contain 64 hexadecimal characters"
        );
        let size = fields.size.as_u64().or_else(|| {
            fields
                .size
                .as_str()
                .and_then(|value| value.parse::<u64>().ok())
        });
        let size = size.context("Invalid installer size")?;
        ensure!(
            size > 0 && size <= MAX_INSTALLER_SIZE,
            "Installer size must be between 1 byte and 1 GiB"
        );
        Ok(Release {
            version,
            installer,
            sha256: fields.sha256.to_ascii_lowercase(),
            size,
        })
    }

    pub fn check(&self, current: Version) -> Result<UpdateCheck> {
        let response = client(Duration::from_secs(30))?
            .get(self.manifest_url.clone())
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
            .send()
            .context("Cannot reach the update service")?;
        self.check_response(response.status(), response, current)
    }

    fn check_response(
        &self,
        status: StatusCode,
        response: impl Read,
        current: Version,
    ) -> Result<UpdateCheck> {
        if status == StatusCode::NOT_FOUND {
            return Ok(UpdateCheck::Unpublished);
        }
        ensure!(status.is_success(), "Update service returned HTTP {status}");
        let mut text = String::new();
        response
            .take(MAX_MANIFEST_SIZE + 1)
            .read_to_string(&mut text)?;
        let release = self.parse_manifest(&text)?;
        Ok(if release.version > current {
            UpdateCheck::Available(release)
        } else {
            UpdateCheck::Current
        })
    }
}

fn client(timeout: Duration) -> Result<Client> {
    Client::builder()
        .https_only(true)
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .user_agent(concat!("Regressify-Desktop/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Cannot create the update client")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DownloadPhase {
    Connecting,
    Downloading,
    Verifying,
    Saving,
}

#[derive(Clone, Debug)]
pub struct DownloadProgress {
    pub phase: DownloadPhase,
    pub received: u64,
    pub total: u64,
    started: Instant,
    last_data: Instant,
}

impl DownloadProgress {
    pub fn message(&self) -> String {
        let elapsed = self.started.elapsed();
        match self.phase {
            DownloadPhase::Connecting => format!(
                "Connecting to update server... {}s elapsed (30s response limit)",
                elapsed.as_secs()
            ),
            DownloadPhase::Downloading => {
                let mib = 1024. * 1024.;
                let rate = self.received as f64 / elapsed.as_secs_f64().max(0.001) / mib;
                let mut message = format!(
                    "Downloading: {:.1} / {:.1} MiB ({}%) | {:.1} MiB/s average | {}s elapsed",
                    self.received as f64 / mib,
                    self.total as f64 / mib,
                    self.received.saturating_mul(100) / self.total.max(1),
                    rate,
                    elapsed.as_secs()
                );
                let idle = self.last_data.elapsed().as_secs();
                if idle >= 5 {
                    message.push_str(&format!(" | No data for {idle}s (30s inactivity limit)"));
                }
                message
            }
            DownloadPhase::Verifying => {
                format!(
                    "Verifying installer SHA-256... {}s elapsed",
                    elapsed.as_secs()
                )
            }
            DownloadPhase::Saving => format!(
                "Saving verified installer to disk... {}s elapsed",
                elapsed.as_secs()
            ),
        }
    }
}

#[derive(Clone)]
pub struct DownloadControl {
    progress: tokio::sync::watch::Sender<DownloadProgress>,
    cancelled: tokio::sync::watch::Sender<bool>,
}

impl DownloadControl {
    pub fn new(release: &Release) -> Self {
        let now = Instant::now();
        let (progress, _) = tokio::sync::watch::channel(DownloadProgress {
            phase: DownloadPhase::Connecting,
            received: 0,
            total: release.size,
            started: now,
            last_data: now,
        });
        let (cancelled, _) = tokio::sync::watch::channel(false);
        Self {
            progress,
            cancelled,
        }
    }

    pub fn progress(&self) -> DownloadProgress {
        self.progress.borrow().clone()
    }

    pub fn cancel(&self) {
        self.cancelled.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    #[cfg(all(test, feature = "gui-tests"))]
    pub(crate) fn set_progress_for_test(&self, phase: DownloadPhase, received: u64) {
        self.update(phase, received);
    }

    fn update(&self, phase: DownloadPhase, received: u64) {
        self.progress.send_modify(|progress| {
            if progress.received != received || progress.phase != phase {
                progress.last_data = Instant::now();
            }
            progress.phase = phase;
            progress.received = received;
        });
    }

    async fn wait_cancelled(&self) {
        let mut cancelled = self.cancelled.subscribe();
        let _ = cancelled.wait_for(|cancelled| *cancelled).await;
    }
}

pub struct PreparedUpdate {
    directory: Option<TempDir>,
    installer: PathBuf,
    release: Release,
}

impl PreparedUpdate {
    #[cfg(all(test, feature = "gui-tests"))]
    pub(crate) fn missing_installer_for_test(release: Release) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        Ok(Self {
            installer: directory.path().join(installer_name(release.version)),
            directory: Some(directory),
            release,
        })
    }

    pub fn download(release: Release, cache: &Path) -> Result<Self> {
        let control = DownloadControl::new(&release);
        Self::download_with_control(release, cache, control)
    }

    pub fn download_with_control(
        release: Release,
        cache: &Path,
        control: DownloadControl,
    ) -> Result<Self> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Cannot start the update downloader")?
            .block_on(async {
                let client = reqwest::Client::builder()
                    .https_only(true)
                    .redirect(Policy::none())
                    .connect_timeout(Duration::from_secs(10))
                    .user_agent(concat!("Regressify-Desktop/", env!("CARGO_PKG_VERSION")))
                    .build()
                    .context("Cannot create the update client")?;
                Self::download_with_client(
                    release,
                    cache,
                    &control,
                    client,
                    DOWNLOAD_IDLE_TIMEOUT,
                    DOWNLOAD_TIMEOUT,
                )
                .await
            })
    }

    async fn download_with_client(
        release: Release,
        cache: &Path,
        control: &DownloadControl,
        client: reqwest::Client,
        idle_timeout: Duration,
        total_timeout: Duration,
    ) -> Result<Self> {
        let download = async {
            fs::create_dir_all(cache).context("Cannot create the update cache")?;
            let directory = tempfile::Builder::new()
                .prefix("regressify-update-")
                .tempdir_in(cache)
                .context("Cannot create the temporary installer directory")?;
            let installer = directory.path().join(installer_name(release.version));
            let mut response = tokio::time::timeout(
                idle_timeout,
                client.get(release.installer.clone()).send(),
            )
            .await
            .with_context(|| format!(
                "Update server did not respond within {} seconds. Check your connection, VPN or proxy, then retry",
                idle_timeout.as_secs_f64()
            ))?
            .context("Cannot connect to the update server; check your connection, VPN or proxy, then retry")?;
            ensure!(
                !response.status().is_redirection(),
                "Installer download was redirected (HTTP {}); update was not installed",
                response.status()
            );
            ensure!(
                response.status().is_success(),
                "Update server returned HTTP {} for the installer; try checking for updates again",
                response.status()
            );
            if let Some(length) = response.content_length() {
                ensure!(
                    length == release.size,
                    "Installer Content-Length is {length} bytes but the manifest declares {} bytes; check for updates again",
                    release.size
                );
            }
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&installer)
                .context(
                    "Cannot save the installer; check free disk space and update-cache permissions",
                )?;
            let mut digest = Sha256::new();
            let mut received = 0u64;
            control.update(DownloadPhase::Downloading, received);
            loop {
                let chunk = tokio::time::timeout(idle_timeout, response.chunk())
                    .await
                    .with_context(|| format!(
                        "Download stalled: no data received for {} seconds ({received} of {} bytes). Check your connection, VPN or proxy, then retry",
                        idle_timeout.as_secs_f64(),
                        release.size
                    ))?
                    .with_context(|| format!(
                        "Installer transfer failed after {received} of {} bytes; check your connection and retry",
                        release.size
                    ))?;
                let Some(chunk) = chunk else {
                    break;
                };
                received += chunk.len() as u64;
                ensure!(
                    received <= release.size,
                    "Installer exceeds its declared size"
                );
                output.write_all(&chunk).context(
                    "Cannot save the installer; check free disk space and update-cache permissions",
                )?;
                digest.update(&chunk);
                control.update(DownloadPhase::Downloading, received);
            }
            control.update(DownloadPhase::Verifying, received);
            verify_digest(received, digest, &release)?;
            control.update(DownloadPhase::Saving, received);
            output.sync_all()
                .context("Cannot finish saving the verified installer; check free disk space and update-cache permissions")?;
            Ok(Self {
                directory: Some(directory),
                installer,
                release,
            })
        };
        tokio::select! {
            biased;
            _ = control.wait_cancelled() => bail!("Download cancelled. No update was installed"),
            result = tokio::time::timeout(total_timeout, download) => result.with_context(|| {
                let progress = control.progress();
                format!(
                    "Update download exceeded its {}-second limit ({} of {} bytes). Check your connection and retry",
                    total_timeout.as_secs_f64(), progress.received, progress.total
                )
            })?,
        }
    }

    pub fn version(&self) -> Version {
        self.release.version
    }

    pub fn launch(&mut self, workspace: Option<&Path>) -> Result<()> {
        ensure!(
            cfg!(target_os = "windows"),
            "Self-update is supported on Windows only"
        );
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        let verified_file = options.open(&self.installer)?;
        verify_payload(verified_file.try_clone()?, std::io::sink(), &self.release)?;
        let mut command = std::process::Command::new(&self.installer);
        command.args(["/SP-", "/SILENT", "/NORESTART"]);
        if let Some(workspace) = workspace {
            command.arg(format!("/WORKSPACE={}", workspace.display()));
        }
        command
            .spawn()
            .context("Cannot start the update installer")?;
        if let Some(directory) = self.directory.take() {
            let _ = directory.keep();
        }
        Ok(())
    }
}

fn verify_payload(mut input: impl Read, mut output: impl Write, release: &Release) -> Result<()> {
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).with_context(|| {
            format!(
                "Installer read failed after {total} of {} bytes",
                release.size
            )
        })?;
        if count == 0 {
            break;
        }
        total += count as u64;
        ensure!(total <= release.size, "Installer exceeds its declared size");
        digest.update(&buffer[..count]);
        output.write_all(&buffer[..count]).context(
            "Cannot save the installer; check free disk space and update-cache permissions",
        )?;
    }
    verify_digest(total, digest, release)
}

fn verify_digest(total: u64, digest: Sha256, release: &Release) -> Result<()> {
    ensure!(
        total == release.size,
        "Installer download is incomplete: received {total} of {} bytes",
        release.size
    );
    if format!("{:x}", digest.finalize()) != release.sha256 {
        bail!("Installer SHA-256 does not match the manifest; update was not installed");
    }
    Ok(())
}

pub fn protect_installation(after_install: bool) -> Result<()> {
    #[cfg(windows)]
    windows::protect_installation(after_install)?;
    #[cfg(not(windows))]
    let _ = after_install;
    Ok(())
}

#[cfg(windows)]
mod windows {
    use std::{
        os::windows::io::{FromRawHandle, OwnedHandle},
        sync::OnceLock,
        time::{Duration, Instant},
    };

    use anyhow::{Context, Result, ensure};
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, GetLastError},
        System::Threading::{CreateMutexW, MUTEX_ALL_ACCESS, OpenMutexW},
    };

    const APP_MUTEX: &str =
        "Local\\Regressify.Desktop.Running.ED6F0908-7E11-4608-A532-E43482AA117B";
    const SETUP_MUTEX: &str =
        "Local\\Regressify.Desktop.Setup.ED6F0908-7E11-4608-A532-E43482AA117B";
    static RUNNING: OnceLock<OwnedHandle> = OnceLock::new();

    fn create_mutex(name: &str) -> Result<OwnedHandle> {
        let name: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        ensure!(
            !handle.is_null(),
            "Cannot register this process with the Windows installer"
        );
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }

    fn mutex_exists(name: &str) -> Result<bool> {
        let name: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe { OpenMutexW(MUTEX_ALL_ACCESS, 0, name.as_ptr()) };
        if !handle.is_null() {
            drop(unsafe { OwnedHandle::from_raw_handle(handle) });
            return Ok(true);
        }
        let error = unsafe { GetLastError() };
        if error == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        Err(std::io::Error::from_raw_os_error(error as i32))
            .context("Cannot check whether setup is running")
    }

    fn acquire(app: &str, setup: &str) -> Result<OwnedHandle> {
        let running = create_mutex(app)?;
        ensure!(
            !mutex_exists(setup)?,
            "Regressify setup is running. Finish setup before opening the app."
        );
        Ok(running)
    }

    pub(super) fn protect_installation(after_install: bool) -> Result<()> {
        if after_install {
            let deadline = Instant::now() + Duration::from_secs(30);
            while mutex_exists(SETUP_MUTEX)? && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        let running = acquire(APP_MUTEX, SETUP_MUTEX)?;
        let _ = RUNNING.set(running);
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{acquire, create_mutex, mutex_exists};

        #[test]
        fn installer_guard_tracks_all_instances_and_blocks_new_launches() {
            let app = format!("Local\\regressify-test-{}", uuid::Uuid::new_v4());
            let setup = format!("{app}-setup");
            let first = acquire(&app, &setup).unwrap();
            let second = acquire(&app, &setup).unwrap();
            drop(first);
            assert!(mutex_exists(&app).unwrap());
            let installing = create_mutex(&setup).unwrap();
            assert!(acquire(&app, &setup).is_err());
            drop(second);
            assert!(!mutex_exists(&app).unwrap());
            drop(installing);
            assert!(acquire(&app, &setup).is_ok());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DownloadControl, DownloadPhase, PreparedUpdate, UpdateCheck, UpdateFeed, Version,
        installer_name, verify_payload,
    };
    use reqwest::StatusCode;
    use sha2::{Digest, Sha256};
    use std::{fs, time::Duration};

    fn manifest(payload: &[u8]) -> String {
        format!(
            "version=1.10.0.0\r\ninstaller=https://drive.tuyen.blog/downloads/regressify/Regressify-Desktop-Setup-1.10.0.0-x64.exe\r\nsha256={:x}\r\nsize={}\r\nnotes=https://example.com/notes\r\n",
            Sha256::digest(payload),
            payload.len()
        )
    }

    #[test]
    fn versions_are_numeric_normalized_and_bounded() {
        assert_eq!("01.03".parse::<Version>().unwrap().to_string(), "1.3.0.0");
        assert!("1.10".parse::<Version>().unwrap() > "1.9.9999".parse::<Version>().unwrap());
        assert_eq!(
            "1.3".parse::<Version>().unwrap(),
            "1.3.0.0".parse().unwrap()
        );
        for invalid in [
            "",
            "v1.2.3",
            "1.2.3-beta",
            "1..2",
            "-1",
            "1.10000",
            "1.2.3.4.5",
            " 1",
            "1.",
        ] {
            assert!(invalid.parse::<Version>().is_err(), "{invalid}");
        }
    }

    #[test]
    fn accepts_text_and_json_manifests() {
        let feed = UpdateFeed::new("regressify").unwrap();
        let text = manifest(b"installer");
        let release = feed.parse_manifest(&text).unwrap();
        assert_eq!(release.version.to_string(), "1.10.0.0");
        let json = serde_json::json!({
            "version": "1.10.0", "installer": release.installer.as_str(),
            "sha256": release.sha256.to_ascii_uppercase(), "size": release.size,
            "pub_date": "2026-09-18T00:00:00.000Z"
        });
        assert_eq!(feed.parse_manifest(&json.to_string()).unwrap(), release);
        assert_eq!(
            feed.parse_manifest(&format!("\u{feff}{text}")).unwrap(),
            release
        );
    }

    #[test]
    fn unpublished_equal_and_older_releases_do_not_update() {
        let feed = UpdateFeed::new("regressify").unwrap();
        let text = manifest(b"installer");
        for version in ["1.10", "2.0"] {
            assert_eq!(
                feed.check_response(StatusCode::OK, text.as_bytes(), version.parse().unwrap())
                    .unwrap(),
                UpdateCheck::Current
            );
        }
        assert!(matches!(
            feed.check_response(StatusCode::OK, text.as_bytes(), "1.9".parse().unwrap())
                .unwrap(),
            UpdateCheck::Available(_)
        ));
        assert_eq!(
            feed.check_response(StatusCode::NOT_FOUND, b"".as_slice(), "1".parse().unwrap())
                .unwrap(),
            UpdateCheck::Unpublished
        );
        for status in [
            StatusCode::FOUND,
            StatusCode::UNAUTHORIZED,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(
                feed.check_response(status, text.as_bytes(), "1".parse().unwrap())
                    .is_err()
            );
        }
    }

    #[test]
    fn rejects_untrusted_urls_and_ambiguous_manifests() {
        let feed = UpdateFeed::new("regressify").unwrap();
        let text = manifest(b"installer");
        for invalid in [
            text.replace("https://drive", "http://drive"),
            text.replace("drive.tuyen.blog/downloads", "evil.example/downloads"),
            text.replace("/regressify/", "/another-app/"),
            text.replace("-x64.exe", "-x64.exe?redirect=elsewhere"),
            text.replace("-x64.exe", "-x64.exe#fragment"),
            text.replace(
                "Regressify-Desktop-Setup-1.10.0.0-x64.exe",
                "latest/installer",
            ),
            text.replace("size=9", "size=0"),
            text.replace("size=9", "size=1073741825"),
            format!("{text}version=2\n"),
            text.replace("sha256=", "hash="),
            "x".repeat(16 * 1024 + 1),
        ] {
            assert!(feed.parse_manifest(&invalid).is_err(), "{invalid}");
        }
        for slug in ["", "../other", "app/path", "app?x", "app\n"] {
            assert!(UpdateFeed::new(slug).is_err());
        }
    }

    #[test]
    fn payload_must_match_both_size_and_digest() {
        let payload = b"installer";
        let release = UpdateFeed::new("regressify")
            .unwrap()
            .parse_manifest(&manifest(payload))
            .unwrap();
        let mut written = Vec::new();
        verify_payload(payload.as_slice(), &mut written, &release).unwrap();
        assert_eq!(written, payload);
        for invalid in [
            b"installed".as_slice(),
            b"short",
            b"installer with extra bytes",
        ] {
            assert!(verify_payload(invalid, std::io::sink(), &release).is_err());
        }
    }

    #[test]
    fn payload_io_errors_identify_the_failed_stage_and_received_bytes() {
        use std::io::{self, Read, Write};

        struct FailedIo;

        impl Read for FailedIo {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "connection stalled",
                ))
            }
        }

        impl Write for FailedIo {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("disk is full"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let payload = b"installer";
        let release = UpdateFeed::new("regressify")
            .unwrap()
            .parse_manifest(&manifest(payload))
            .unwrap();
        let error =
            verify_payload(b"ins".as_slice().chain(FailedIo), io::sink(), &release).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("after 3 of 9 bytes"), "{message}");
        assert!(message.contains("connection stalled"), "{message}");
        let error = verify_payload(payload.as_slice(), FailedIo, &release).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("Cannot save the installer"), "{message}");
        assert!(message.contains("disk is full"), "{message}");
    }

    async fn download_server(
        response: &str,
    ) -> (
        reqwest::Url,
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/installer", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let response = response.to_owned();
        let (started, request_received) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(stream.read_u8().await.unwrap());
                assert!(request.len() < 16 * 1024);
            }
            stream.write_all(response.as_bytes()).await.unwrap();
            let _ = started.send(());
            futures::future::pending::<()>().await;
        });
        (url, server, request_received)
    }

    #[tokio::test]
    async fn stalled_headers_and_body_time_out_and_remove_partial_downloads() {
        for (response, received, expected_error) in [
            ("", 0, "Update server did not respond"),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nins",
                3,
                "Download stalled",
            ),
        ] {
            let (url, server, _) = download_server(response).await;
            let mut release = UpdateFeed::new("regressify")
                .unwrap()
                .parse_manifest(&manifest(b"installer"))
                .unwrap();
            release.installer = url;
            let control = DownloadControl::new(&release);
            let cache = tempfile::tempdir().unwrap();
            let result = PreparedUpdate::download_with_client(
                release,
                cache.path(),
                &control,
                reqwest::Client::new(),
                Duration::from_millis(200),
                Duration::from_secs(3),
            )
            .await;
            server.abort();
            let error = format!("{:#}", result.err().expect("stalled download must fail"));
            assert!(error.contains(expected_error), "{error}");
            assert!(error.contains("retry"), "{error}");
            assert_eq!(control.progress().received, received);
            assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
            if received > 0 {
                assert!(error.contains("3 of 9 bytes"), "{error}");
            }
        }
    }

    #[tokio::test]
    async fn cancellation_interrupts_waiting_for_the_server_and_cleans_up() {
        let (url, server, started) = download_server("").await;
        let mut release = UpdateFeed::new("regressify")
            .unwrap()
            .parse_manifest(&manifest(b"installer"))
            .unwrap();
        release.installer = url;
        let control = DownloadControl::new(&release);
        let cache = tempfile::tempdir().unwrap();
        let download = PreparedUpdate::download_with_client(
            release,
            cache.path(),
            &control,
            reqwest::Client::new(),
            Duration::from_secs(30),
            Duration::from_secs(60),
        );
        let cancel = async {
            started.await.unwrap();
            control.cancel();
        };
        let (result, ()) = tokio::time::timeout(Duration::from_secs(3), async {
            tokio::join!(download, cancel)
        })
        .await
        .expect("cancellation must not wait for the network timeout");
        server.abort();
        let error = format!("{:#}", result.err().expect("cancelled download must fail"));
        assert!(error.contains("Download cancelled"), "{error}");
        assert!(control.is_cancelled());
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn streamed_installer_still_requires_success_size_and_sha256() {
        for (response, expected) in [
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\ninstaller",
                None,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\ninstalled",
                Some("SHA-256"),
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nins",
                Some("Content-Length"),
            ),
            (
                "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n",
                Some("HTTP 503"),
            ),
            (
                "HTTP/1.1 302 Found\r\nLocation: https://example.com/\r\nContent-Length: 0\r\n\r\n",
                Some("redirected"),
            ),
            (
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nins\r\n0\r\n\r\n",
                Some("incomplete"),
            ),
            (
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nA\r\ninstallers\r\n0\r\n\r\n",
                Some("exceeds"),
            ),
        ] {
            let (url, server, _) = download_server(response).await;
            let mut release = UpdateFeed::new("regressify")
                .unwrap()
                .parse_manifest(&manifest(b"installer"))
                .unwrap();
            release.installer = url;
            let control = DownloadControl::new(&release);
            let cache = tempfile::tempdir().unwrap();
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap();
            let result = PreparedUpdate::download_with_client(
                release,
                cache.path(),
                &control,
                client,
                Duration::from_secs(2),
                Duration::from_secs(3),
            )
            .await;
            server.abort();
            match expected {
                Some(expected) => {
                    let error = format!("{:#}", result.err().expect("invalid download must fail"));
                    assert!(error.contains(expected), "{error}");
                }
                None => {
                    let prepared = result.unwrap();
                    assert_eq!(fs::read(&prepared.installer).unwrap(), b"installer");
                    assert_eq!(control.progress().received, 9);
                    assert_eq!(control.progress().phase, DownloadPhase::Saving);
                }
            }
            assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
        }
    }

    #[tokio::test]
    async fn total_deadline_is_independent_of_the_inactivity_limit() {
        let (url, server, _) = download_server("").await;
        let mut release = UpdateFeed::new("regressify")
            .unwrap()
            .parse_manifest(&manifest(b"installer"))
            .unwrap();
        release.installer = url;
        let control = DownloadControl::new(&release);
        let cache = tempfile::tempdir().unwrap();
        let result = PreparedUpdate::download_with_client(
            release,
            cache.path(),
            &control,
            reqwest::Client::new(),
            Duration::from_secs(3),
            Duration::from_millis(100),
        )
        .await;
        server.abort();
        let error = format!(
            "{:#}",
            result.err().expect("total deadline must be enforced")
        );
        assert!(error.contains("exceeded its 0.1-second limit"), "{error}");
        assert!(error.contains("0 of 9 bytes"), "{error}");
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }

    #[test]
    fn progress_distinguishes_network_inactivity_verification_and_disk_saving() {
        let release = UpdateFeed::new("regressify")
            .unwrap()
            .parse_manifest(&manifest(b"installer"))
            .unwrap();
        let control = DownloadControl::new(&release);
        assert!(control.progress().message().contains("Connecting"));
        control.update(DownloadPhase::Downloading, 3);
        let mut progress = control.progress();
        progress.started -= Duration::from_secs(12);
        progress.last_data -= Duration::from_secs(8);
        let message = progress.message();
        assert!(message.contains("33%"), "{message}");
        assert!(message.contains("MiB/s"), "{message}");
        assert!(message.contains("12s elapsed"), "{message}");
        assert!(message.contains("No data for 8s"), "{message}");
        control.update(DownloadPhase::Verifying, 9);
        assert!(control.progress().message().contains("SHA-256"));
        control.update(DownloadPhase::Saving, 9);
        assert!(
            control
                .progress()
                .message()
                .contains("Saving verified installer to disk")
        );
    }

    #[test]
    fn abandoned_download_is_removed_and_tampering_blocks_launch() {
        let payload = b"installer";
        let release = UpdateFeed::new("regressify")
            .unwrap()
            .parse_manifest(&manifest(payload))
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().to_path_buf();
        let installer = path.join(installer_name(release.version));
        fs::write(&installer, b"installed").unwrap();
        let mut update = PreparedUpdate {
            directory: Some(directory),
            installer,
            release,
        };
        assert!(update.launch(None).is_err());
        drop(update);
        assert!(!path.exists());
    }
}
