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
pub static NO_PRINT: OnceLock<bool> = OnceLock::new();
pub static VERBOSE: OnceLock<bool> = OnceLock::new();
pub static PROXY: OnceLock<Proxy> = OnceLock::new();
