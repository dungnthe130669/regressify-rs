use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
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
        fs::create_dir_all(cache).context("Cannot create the update cache")?;
        let directory = tempfile::Builder::new()
            .prefix("regressify-update-")
            .tempdir_in(cache)?;
        let installer = directory.path().join(installer_name(release.version));
        let response = client(Duration::from_secs(15 * 60))?
            .get(release.installer.clone())
            .send()
            .context("Cannot download the installer")?
            .error_for_status()
            .context("Installer download failed")?;
        ensure!(
            response.status().is_success(),
            "Installer download was redirected"
        );
        if let Some(length) = response.content_length() {
            ensure!(
                length == release.size,
                "Installer Content-Length does not match the manifest"
            );
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&installer)?;
        verify_payload(response, &mut output, &release)?;
        output.sync_all()?;
        Ok(Self {
            directory: Some(directory),
            installer,
            release,
        })
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
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        ensure!(total <= release.size, "Installer exceeds its declared size");
        digest.update(&buffer[..count]);
        output.write_all(&buffer[..count])?;
    }
    ensure!(total == release.size, "Installer download is incomplete");
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
    use super::{PreparedUpdate, UpdateCheck, UpdateFeed, Version, installer_name, verify_payload};
    use reqwest::StatusCode;
    use sha2::{Digest, Sha256};
    use std::fs;

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
