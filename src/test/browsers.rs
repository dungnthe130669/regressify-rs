use std::path::PathBuf;
use std::time::Duration;

use anyhow::anyhow;
use base64::{Engine as _, engine::general_purpose};
use chromiumoxide::{
    Browser, BrowserConfig,
    fetcher::{BrowserFetcher, BrowserFetcherOptions},
    handler::viewport::Viewport,
};
use futures::StreamExt;
use tokio::task::JoinHandle;

use crate::{
    common,
    helpers::{OnceLockBoolExtensions, OnceLockExtensions},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct LaunchOptions {
    pub debug: bool,
    pub ignore_ssl_errors: bool,
}

pub async fn fetcher() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let work_dir = common::ROOT_DIR.get_required()?;

    let download_path = work_dir.join("download");
    tokio::fs::create_dir_all(&download_path).await?;
    let fetcher = BrowserFetcher::new(
        BrowserFetcherOptions::builder()
            .with_path(&download_path)
            .build()?,
    );
    let info = fetcher.fetch().await?;
    let exe = &info.executable_path;

    if !exe.exists() {
        return Err(anyhow!(
            "Fetched browser executable does not exist: {}",
            exe.display()
        )
        .into());
    }

    // Ensure the binary is executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(exe)?;
        let perms = metadata.permissions();
        if perms.mode() & 0o111 == 0 {
            eprintln!("Fixing executable permissions for: {}", exe.display());
            std::fs::set_permissions(exe, std::fs::Permissions::from_mode(perms.mode() | 0o755))?;
        }
    }

    eprintln!("Browser executable: {}", exe.display());
    Ok(info.executable_path)
}

/// Resolve a valid Chrome/Chromium executable path.
/// If `exe_path` is provided but missing on disk, falls back to auto-detection.
fn resolve_executable(exe_path: &Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = exe_path {
        if path.exists() {
            return Some(path.clone());
        }
        eprintln!(
            "Warning: provided browser path does not exist: {}. Falling back to auto-detection.",
            path.display()
        );
    }
    None
}

const MAX_LAUNCH_RETRIES: u32 = 2;

pub async fn launch_browser(
    exe_path: &Option<PathBuf>,
    log_prefix: &str,
    options: LaunchOptions,
) -> Result<(Browser, JoinHandle<()>, PathBuf), Box<dyn std::error::Error>> {
    let debug = common::DEBUG.get_bool() || options.debug;

    let mut config_builder = BrowserConfig::builder();

    let resolved = resolve_executable(exe_path);
    if let Some(ref resolved) = resolved {
        config_builder = config_builder.chrome_executable(resolved);
    }

    if debug {
        config_builder = config_builder.with_head();
    }

    // Use a unique user data dir per launch to avoid ProcessSingleton lock conflicts
    let guid_base64url = general_purpose::URL_SAFE_NO_PAD.encode(uuid::Uuid::new_v4().as_bytes());
    let user_data_dir = std::env::temp_dir().join(format!("regressify-browser-{}", guid_base64url));
    config_builder = config_builder.user_data_dir(&user_data_dir);

    let mut args = vec![
        "--disable-infobars",
        "--disable-setuid-sandbox",
        "--no-sandbox",
        "--window-position=0,0",
    ];
    if options.ignore_ssl_errors {
        args.push("--ignore-certificate-errors");
        args.push("--ignore-certificate-errors-spki-list");
    }
    if !debug {
        // New headless mode (`--headless=new`) runs the full Chrome stack and
        // initializes the Ozone platform, which on Linux defaults to the X11
        // backend and therefore needs an X server / $DISPLAY. On headless CI
        // agents this intermittently fails with "Missing X server or $DISPLAY"
        // under concurrent launches. Pin the Ozone backend to `headless` so the
        // browser never touches X11.
        args.push("--ozone-platform=headless");
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

    if let Some(ref path) = resolved {
        eprintln!(
            "{} Launching browser (executable: {})...",
            log_prefix,
            path.display()
        );
    } else {
        eprintln!("{} Launching browser (auto-detected)...", log_prefix);
    }

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
                eprintln!(
                    "{} Browser launch attempt {}/{} failed: {:?}",
                    log_prefix, attempt, MAX_LAUNCH_RETRIES, e
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
