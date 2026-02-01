use std::path::PathBuf;

use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};

use crate::{common, helpers::OnceLockExtensions};

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
