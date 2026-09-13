#![allow(dead_code)]

use reqwest::Proxy;
use std::{path::PathBuf, sync::OnceLock};

pub const COMPANY_NAME: &str = "regressify";
pub const APP_NAME: &str = "cli";

#[cfg(target_os = "windows")]
pub const SEVEN_ZIP_PATH: &str = "C:/Program Files/7-Zip/7z.exe";

#[cfg(not(target_os = "windows"))]
pub const SEVEN_ZIP_PATH: &str = "7z";

pub static ROOT_DIR: OnceLock<PathBuf> = OnceLock::new();
pub static REGRESSIFY_DIR: OnceLock<PathBuf> = OnceLock::new();
pub static RUN_ID: OnceLock<String> = OnceLock::new();
pub static NO_PRINT: OnceLock<bool> = OnceLock::new();
pub static VERBOSE: OnceLock<bool> = OnceLock::new();
pub static PROXY: OnceLock<Proxy> = OnceLock::new();
pub static DEBUG: OnceLock<bool> = OnceLock::new();
pub static REQUIRED_LOGIN: OnceLock<bool> = OnceLock::new();
pub static ASYNC_CAPTURE_LIMIT: OnceLock<usize> = OnceLock::new();
pub static ASYNC_CAPTURE_RAMP_MS: OnceLock<u64> = OnceLock::new();

/// Whether the process is running in a CI environment.
///
/// Mirrors the TypeScript `isCIEnvironment()` helper: a generic `CI=true`, an
/// Azure Pipelines run, or a GitHub Actions run all count as CI. Used to force
/// headed/debug browser launches off, since CI agents have no display.
pub fn is_ci() -> bool {
    std::env::var("CI").unwrap_or_default() == "true"
        || crate::vso::is_azure_devops()
        || crate::github::is_github()
}
