//! Shared configuration, automation, and reporting used by the CLI and desktop.

pub mod analytics;
pub mod application;
pub mod cli;
pub mod common;
pub mod compare;
pub mod config;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod diagnostics;
pub mod github;
pub mod helpers;
pub mod init;
pub mod optimizer;
pub mod progress;
pub mod snapshot;
pub mod test;
pub mod types;
pub mod vso;
