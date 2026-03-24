use std::path::PathBuf;
use std::time::Duration;

use anyhow::anyhow;
use base64::{Engine as _, engine::general_purpose};
use chromiumoxide::{
    Browser, BrowserConfig,
    browser::HeadlessMode,
    fetcher::{BrowserFetcher, BrowserFetcherOptions},
    handler::viewport::Viewport,
};
use futures::StreamExt;
use tokio::task::JoinHandle;

use crate::{
    common,
    helpers::{OnceLockBoolExtensions, OnceLockExtensions},
};

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
) -> Result<(Browser, JoinHandle<()>), Box<dyn std::error::Error>> {
    let debug = common::DEBUG.get_bool();

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
        .args([
            "--disable-infobars",
            "--disable-setuid-sandbox",
            "--ignore-certificate-errors",
            "--ignore-certificate-errors-spki-list",
            "--no-sandbox",
            "--window-position=0,0",
        ]);

    if debug {
        config_builder = config_builder.with_head();
    } else {
        config_builder = config_builder.headless_mode(HeadlessMode::New);
    }

    let config = config_builder
        .build()
        .map_err(|e| anyhow!("Cannot create browser config: {:?}", e))?;

    if let Some(ref path) = resolved {
        eprintln!("Launching browser (executable: {})...", path.display());
    } else {
        eprintln!("Launching browser (auto-detected)...");
    }

    let mut last_err = None;
    for attempt in 1..=MAX_LAUNCH_RETRIES {
        match Browser::launch(config.clone()).await {
            Ok((browser, mut handler)) => {
                let data_dir = user_data_dir.clone();
                let handle = tokio::spawn(async move {
                    // Process CDP events with a timeout to prevent indefinite hangs
                    loop {
                        match tokio::time::timeout(Duration::from_secs(30), handler.next()).await {
                            Ok(Some(h)) if h.is_err() => break,
                            Ok(None) => break,
                            Ok(Some(_)) => {} // Continue processing events
                            Err(_) => {
                                // Timeout waiting for next event, exit cleanup loop
                                break;
                            }
                        }
                    }
                    let _ = tokio::fs::remove_dir_all(&data_dir).await;
                });
                return Ok((browser, handle));
            }
            Err(e) => {
                eprintln!(
                    "Browser launch attempt {}/{} failed: {:?}",
                    attempt, MAX_LAUNCH_RETRIES, e
                );
                last_err = Some(e);
                if attempt < MAX_LAUNCH_RETRIES {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(anyhow!(
        "Cannot launch browser after {} attempts: {:?}",
        MAX_LAUNCH_RETRIES,
        last_err,
    )
    .into())
}
