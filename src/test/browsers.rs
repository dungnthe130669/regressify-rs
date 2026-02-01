use std::path::PathBuf;

use anyhow::anyhow;
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

    Ok(info.executable_path)
}

pub async fn launch_browser(
    exe_path: &Option<PathBuf>,
) -> Result<(Browser, JoinHandle<()>), Box<dyn std::error::Error>> {
    let debug = common::DEBUG.get_bool();

    // create a `Browser` that spawns a `chromium` process running with UI (`with_head()`, headless is default)
    // and the handler that drives the websocket etc.
    let mut config_builder = BrowserConfig::builder();

    if let Some(exe_path) = exe_path {
        config_builder = config_builder.chrome_executable(&exe_path);
    }

    if debug {
        config_builder = config_builder.with_head();
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
        .args([
            "--disable-infobars",
            "--disable-setuid-sandbox",
            "--ignore-certificate-errors",
            "--ignore-certificate-errors-spki-list",
            "--no-sandbox",
            // "--disable-setuid-sandbox",
            // "--disable-gpu",
            "--window-position=0,0",
        ]);

    if common::DEBUG.get_bool() {
        config_builder = config_builder.with_head();
    } else {
        config_builder = config_builder.headless_mode(HeadlessMode::New);
    }

    let config = config_builder
        .build()
        .map_err(|e| anyhow!("Cannot create browser config: {:?}", e))?;

    eprintln!("Launching browser...");
    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| anyhow!("Cannot launch browser: {:?}", e))?;

    // spawn a new task that continuously polls the handler
    let handle = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    Ok((browser, handle))
}
