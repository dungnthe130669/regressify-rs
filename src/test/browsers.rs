use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, anyhow};
use base64::{Engine as _, engine::general_purpose};
use chromiumoxide::{
    Browser, BrowserConfig,
    fetcher::{BrowserHost, BrowserKind, BrowserVersion, BuildInfo, Platform},
    handler::viewport::Viewport,
};
use futures::StreamExt;
use tokio::task::JoinHandle;

use crate::{
    common,
    helpers::OnceLockBoolExtensions,
    progress::{self, Event as ProgressEvent},
};

mod install;

#[derive(Clone, Copy, Debug, Default)]
pub struct LaunchOptions {
    pub debug: bool,
    pub ignore_ssl_errors: bool,
}

pub async fn fetcher() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(prepare_browser(Some(true)).await?)
}

fn browser_cache_dir() -> anyhow::Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|directories| directories.cache_dir().join("regressify/chromium"))
        .ok_or_else(|| anyhow!("Cannot determine the shared Chromium cache directory"))
}

struct ManagedBrowser {
    directory: PathBuf,
    executable: PathBuf,
    url: String,
}

fn managed_browser(cache_dir: &Path) -> anyhow::Result<ManagedBrowser> {
    let version = BrowserVersion::current(BrowserKind::Chromium);
    let BrowserVersion::Revision(revision) = version else {
        return Err(anyhow!(
            "Managed Chromium requires a fixed browser revision"
        ));
    };
    let (platform, folder, executable) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => (Platform::Linux, "linux", "chrome-linux/chrome"),
        ("macos", "x86_64") => (
            Platform::Mac,
            "mac",
            "chrome-mac/Chromium.app/Contents/MacOS/Chromium",
        ),
        ("macos", "aarch64") => (
            Platform::MacArm,
            "mac_arm",
            "chrome-mac/Chromium.app/Contents/MacOS/Chromium",
        ),
        ("windows", "x86") => (Platform::Win32, "win32", "chrome-win/chrome.exe"),
        ("windows", "x86_64" | "aarch64") => (Platform::Win64, "win64", "chrome-win/chrome.exe"),
        (os, arch) => return Err(anyhow!("Managed Chromium is not supported on {os}/{arch}")),
    };
    let directory = cache_dir.join(format!("{folder}-{revision}"));
    Ok(ManagedBrowser {
        executable: directory.join(executable),
        directory,
        url: BrowserKind::Chromium.download_url(
            platform,
            &BuildInfo::revision(revision),
            &BrowserHost::current(BrowserKind::Chromium),
        ),
    })
}

const BROWSER_LOG_INTERVAL: Duration = Duration::from_secs(5);
const BROWSER_LOCK_TIMEOUT: Duration = Duration::from_secs(5 * 60);

fn browser_status(message: impl Into<String>) {
    let message = message.into();
    crate::progress::scenario_log!("{message}");
    progress::emit(ProgressEvent::Phase { name: message });
}

async fn lock_browser_cache(cache_dir: &Path) -> anyhow::Result<std::fs::File> {
    lock_browser_cache_with_timeout(cache_dir, BROWSER_LOCK_TIMEOUT).await
}

async fn lock_browser_cache_with_timeout(
    cache_dir: &Path,
    timeout: Duration,
) -> anyhow::Result<std::fs::File> {
    let lock_path = cache_dir.join(".install.lock");
    let lock = tokio::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .await
        .with_context(|| {
            format!(
                "Cannot open shared Chromium cache lock: {}. Check cache folder permissions.",
                lock_path.display()
            )
        })?
        .into_std()
        .await;
    let started = tokio::time::Instant::now();
    let deadline = started + timeout;
    let mut next_log = started;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(std::fs::TryLockError::WouldBlock) => {}
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(error).context("Cannot lock the shared Chromium cache");
            }
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(anyhow!(
                "Timed out after {:.0}s waiting for the Chromium cache lock: {}. Another Regressify process may still be installing Chromium. Close or cancel that run, then retry; do not delete the lock file while a run is active.",
                started.elapsed().as_secs_f64(),
                lock_path.display()
            ));
        }
        if now >= next_log {
            browser_status(format!(
                "Waiting for Chromium cache lock ({:.0}s elapsed; another Regressify process is preparing the browser)",
                started.elapsed().as_secs_f64()
            ));
            next_log = now + BROWSER_LOG_INTERVAL;
        }
        tokio::time::sleep_until((now + Duration::from_millis(100)).min(deadline)).await;
    }
}

pub(super) async fn prepare_browser(fetch_browser: Option<bool>) -> anyhow::Result<PathBuf> {
    prepare_browser_in(&browser_cache_dir()?, fetch_browser).await
}

async fn prepare_browser_in(
    cache_dir: &Path,
    fetch_browser: Option<bool>,
) -> anyhow::Result<PathBuf> {
    browser_status("Checking Chromium cache");
    let browser = managed_browser(cache_dir)?;
    let cached_executable = &browser.executable;
    crate::progress::scenario_log!(
        "Shared Chromium cache: {} (downloads {})",
        cache_dir.display(),
        if fetch_browser == Some(false) {
            "disabled"
        } else {
            "enabled"
        }
    );
    if fetch_browser == Some(false) && !cached_executable.is_file() {
        return resolve_executable(&Some(cached_executable.clone()));
    }
    tokio::fs::create_dir_all(cache_dir)
        .await
        .with_context(|| {
            format!(
                "Cannot create Chromium cache: {}. Check disk space and folder permissions.",
                cache_dir.display()
            )
        })?;
    let cache_lock = std::sync::Arc::new(lock_browser_cache(cache_dir).await?);
    browser_status("Checking cached Chromium executable");
    if cached_executable.is_file() {
        crate::progress::scenario_log!("Reusing cached Chromium; no download needed");
    } else if fetch_browser.unwrap_or(true) {
        install::install_browser(&browser, cache_lock.clone()).await?;
    } else {
        return resolve_executable(&Some(cached_executable.clone()));
    }
    let exe = resolve_executable(&Some(cached_executable.clone()))?;

    // Ensure the binary is executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&exe)?;
        let perms = metadata.permissions();
        if perms.mode() & 0o111 == 0 {
            crate::progress::scenario_log!("Fixing executable permissions for: {}", exe.display());
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(perms.mode() | 0o755))?;
        }
    }

    browser_status("Chromium ready");
    crate::progress::scenario_log!("Managed Chromium executable: {}", exe.display());
    Ok(exe)
}

fn resolve_executable(exe_path: &Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let path = exe_path.as_ref().ok_or_else(|| {
        anyhow!("Managed Chromium executable was not selected. Run 'regressify install' before capturing.")
    })?;
    if !path.is_file() {
        return Err(anyhow!(
            "Managed Chromium executable is missing or is not a file: {}. Run 'regressify install' to repair the browser cache.",
            path.display()
        ));
    }
    Ok(path.clone())
}

const MAX_LAUNCH_RETRIES: u32 = 2;

pub async fn launch_browser(
    exe_path: &Option<PathBuf>,
    log_prefix: &str,
    options: LaunchOptions,
) -> Result<(Browser, JoinHandle<()>, PathBuf), Box<dyn std::error::Error>> {
    let debug = (common::DEBUG.get_bool() || options.debug) && !common::is_ci();

    let resolved = resolve_executable(exe_path)?;
    let mut config_builder = BrowserConfig::builder().chrome_executable(&resolved);

    if debug {
        config_builder = config_builder.with_head();
    }

    // Use a unique user data dir per launch to avoid ProcessSingleton lock conflicts
    let guid_base64url = general_purpose::URL_SAFE_NO_PAD.encode(uuid::Uuid::new_v4().as_bytes());
    let user_data_dir = std::env::temp_dir().join(format!("regressify-browser-{}", guid_base64url));
    config_builder = config_builder.user_data_dir(&user_data_dir);

    let mut args = vec![
        // The four explicit args the Node.js version passed to Playwright via
        // `engineOptions.args` (its two "--ignore-certifcate-errors" flags were
        // misspelled no-ops; we handle SSL correctly below).
        "--disable-infobars",
        "--disable-setuid-sandbox",
        "--no-sandbox",
        "--window-position=0,0",
        // Rendering-determinism flags. The biggest determinism win is the
        // capture method itself — see `save_screenshot` in capture.rs, which
        // captures with `captureBeyondViewport` instead of resizing the
        // viewport. These flags pin the remaining sources of pixel drift.
        //
        // IMPORTANT: Chrome honors only the LAST `--enable-features` /
        // `--disable-features` switch on the command line; duplicates are not
        // merged. chromiumoxide's DEFAULT_ARGS already pass
        // `--enable-features=NetworkService,NetworkServiceInProcess` and
        // `--disable-features=TranslateUI`, and our args are appended after
        // them — so we must restate those values here or they get dropped.
        //
        // - CDPScreenshotNewSurface: Playwright's screenshot determinism feature
        //   — captures from a dedicated surface, independent of on-screen
        //   compositing state.
        // - PaintHolding (disabled): Playwright disables it so first paint is
        //   not deferred, giving predictable paint timing before capture.
        // - force-color-profile=srgb is already provided by chromiumoxide's
        //   DEFAULT_ARGS, so we do not repeat it.
        // - font-render-hinting / skia-runtime-opts / partial-raster pin glyph
        //   rasterization and tile reuse (extra hardening beyond Playwright).
        //
        // NOTE: we intentionally do NOT pass --disable-gpu / --use-gl=swiftshader.
        // Playwright keeps GPU rasterization enabled and is still deterministic;
        // those flags are not what makes screenshots stable and can interfere
        // with the new-surface capture path.
        "--enable-features=NetworkService,NetworkServiceInProcess,CDPScreenshotNewSurface",
        "--disable-features=TranslateUI,PaintHolding",
        "--font-render-hinting=none",
        "--disable-skia-runtime-opts",
        "--disable-partial-raster",
    ];
    if options.ignore_ssl_errors {
        args.push("--ignore-certificate-errors");
        args.push("--ignore-certificate-errors-spki-list");
    }

    config_builder = config_builder
        .window_size(1920, 1080)
        .viewport(Some(Viewport {
            width: 1920,
            height: 1080,
            device_scale_factor: Some(1.0),
            emulating_mobile: false,
            has_touch: false,
            is_landscape: true,
        }))
        .launch_timeout(Duration::from_secs(45))
        .args(args);

    if debug {
        config_builder = config_builder.with_head();
    } else {
        config_builder = config_builder.new_headless_mode();
    }

    let config = config_builder
        .build()
        .map_err(|e| anyhow!("Cannot create browser config: {:?}", e))?;

    crate::progress::scenario_log!(
        "{} Launching Chromium (executable: {})...",
        log_prefix,
        resolved.display()
    );

    let mut last_err = None;
    for attempt in 1..=MAX_LAUNCH_RETRIES {
        match Browser::launch(config.clone()).await {
            Ok((browser, mut handler)) => {
                let data_dir = user_data_dir.clone();
                let handle = tokio::spawn(async move {
                    // Pump CDP events for the full lifetime of the browser. The loop
                    // ends naturally when the browser is closed/dropped (stream yields
                    // None) or on a transport error. Do NOT use an idle timeout here:
                    // long waits (e.g. `delay` after interactions) produce no CDP
                    // events, and prematurely ending this task tears down the command
                    // channel, causing later commands to fail with "oneshot canceled"
                    // / "send failed because receiver is gone".
                    while let Some(h) = handler.next().await {
                        if h.is_err() {
                            break;
                        }
                    }
                    let _ = tokio::fs::remove_dir_all(&data_dir).await;
                });
                return Ok((browser, handle, user_data_dir));
            }
            Err(e) => {
                crate::progress::scenario_log!(
                    "{} Browser launch attempt {}/{} failed: {:?}",
                    log_prefix,
                    attempt,
                    MAX_LAUNCH_RETRIES,
                    e
                );
                last_err = Some(e);
                if attempt < MAX_LAUNCH_RETRIES {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    // All retries exhausted — clean up the leaked user-data directory before failing.
    let _ = std::fs::remove_dir_all(&user_data_dir);

    Err(anyhow!(
        "Cannot launch browser after {} attempts: {:?}",
        MAX_LAUNCH_RETRIES,
        last_err,
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::{
        browser_cache_dir, lock_browser_cache, lock_browser_cache_with_timeout, managed_browser,
        prepare_browser_in, resolve_executable,
    };

    #[test]
    fn managed_chromium_uses_the_os_user_cache() {
        let directories = directories::BaseDirs::new().unwrap();
        assert_eq!(
            browser_cache_dir().unwrap(),
            directories.cache_dir().join("regressify/chromium")
        );
        assert!(browser_cache_dir().unwrap().is_absolute());
    }

    #[tokio::test]
    async fn shared_browser_cache_lock_is_exclusive_and_released() {
        let directory = tempfile::tempdir().unwrap();
        let lock = lock_browser_cache(directory.path()).await.unwrap();
        let contender = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(directory.path().join(".install.lock"))
            .unwrap();
        assert!(contender.try_lock().is_err());
        drop(lock);
        contender.try_lock().unwrap();
    }

    #[tokio::test]
    async fn shared_browser_cache_lock_times_out_without_leaving_a_waiter() {
        let directory = tempfile::tempdir().unwrap();
        let lock = lock_browser_cache(directory.path()).await.unwrap();
        let error =
            lock_browser_cache_with_timeout(directory.path(), std::time::Duration::from_millis(20))
                .await
                .unwrap_err()
                .to_string();
        assert!(error.contains("Timed out"));
        assert!(error.contains("another Regressify") || error.contains("Another Regressify"));
        assert!(error.contains(&directory.path().display().to_string()));
        drop(lock);
        lock_browser_cache_with_timeout(directory.path(), std::time::Duration::ZERO)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn all_download_settings_use_the_same_managed_chromium() {
        let directory = tempfile::tempdir().unwrap();
        let browser = managed_browser(directory.path()).unwrap();
        let executable = browser.executable;
        let fetcher = chromiumoxide::fetcher::BrowserFetcher::new(
            chromiumoxide::fetcher::BrowserFetcherOptions::builder()
                .with_path(directory.path())
                .build()
                .unwrap(),
        );
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, "cached Chromium fixture").unwrap();
        assert_eq!(fetcher.fetch().await.unwrap().executable_path, executable);

        let (automatic, enabled, disabled) = tokio::join!(
            prepare_browser_in(directory.path(), None),
            prepare_browser_in(directory.path(), Some(true)),
            prepare_browser_in(directory.path(), Some(false)),
        );
        for result in [automatic, enabled, disabled] {
            assert_eq!(result.unwrap(), executable);
        }
        assert_eq!(
            std::fs::read_to_string(executable).unwrap(),
            "cached Chromium fixture"
        );
    }

    #[tokio::test]
    async fn downloads_disabled_requires_the_pinned_cached_browser() {
        let directory = tempfile::tempdir().unwrap();
        let executable = managed_browser(directory.path()).unwrap().executable;
        let error = prepare_browser_in(directory.path(), Some(false))
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(&executable.display().to_string()));
        assert!(error.contains("regressify install"));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);

        let stale = directory.path().join("stale-build/chrome.exe");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(stale, "wrong revision").unwrap();
        assert!(
            prepare_browser_in(directory.path(), Some(false))
                .await
                .is_err()
        );

        std::fs::create_dir_all(executable).unwrap();
        assert!(
            prepare_browser_in(directory.path(), Some(false))
                .await
                .is_err()
        );
    }

    #[test]
    fn browser_launch_requires_an_explicit_executable() {
        let error = resolve_executable(&None).unwrap_err().to_string();
        assert!(error.contains("Managed Chromium executable was not selected"));
        assert!(error.contains("regressify install"));
    }

    #[test]
    fn browser_launch_rejects_missing_executables_and_directories() {
        let directory = tempfile::tempdir().unwrap();
        for path in [
            directory.path().join("missing.exe"),
            directory.path().to_path_buf(),
        ] {
            let error = resolve_executable(&Some(path.clone()))
                .unwrap_err()
                .to_string();
            assert!(error.contains(&path.display().to_string()));
            assert!(error.contains("regressify install"));
        }
    }

    #[test]
    fn browser_launch_preserves_the_selected_executable() {
        let executable = tempfile::NamedTempFile::new().unwrap();
        let path = executable.path().to_path_buf();
        assert_eq!(resolve_executable(&Some(path.clone())).unwrap(), path);
    }
}
