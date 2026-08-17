use clap::Parser;
use std::path::PathBuf;

use super::Command;

/// Version string shown by `--version`, including the embedded git commit hash.
const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("BINARY_GIT_HASH"),
    ")"
);

#[derive(Parser)]
#[clap(version = LONG_VERSION, author, about)]
pub struct Cli {
    /// Directory to use as root of project
    #[clap(short = 'r', long, default_value = ".", allow_hyphen_values = true)]
    pub root: PathBuf,

    /// Pause the program after execution
    #[clap(long, action, visible_alias = "wait")]
    pub pause: bool,

    /// Pause the program after execution if an error occurs
    #[clap(long, action, visible_alias = "wait-if-error")]
    pub pause_if_error: bool,

    /// Proxy server to use
    #[clap(long, allow_hyphen_values = true)]
    pub proxy: Option<String>,

    /// Do not print output
    #[clap(long, action, default_value = "false")]
    pub no_print: bool,

    /// Verbose output
    #[clap(long, default_value = "false")]
    pub verbose: bool,

    /// Site environment
    #[clap(long, allow_hyphen_values = true)]
    pub site_env: Option<String>,

    /// Directory to use as regressify directory (overrides site_env)
    #[clap(long, allow_hyphen_values = true)]
    pub regressify_dir: Option<PathBuf>,

    /// Run ID
    #[clap(long, allow_hyphen_values = true)]
    pub run_id: Option<String>,

    /// Number of test scenarios to run in parallel
    #[clap(long, allow_hyphen_values = true, default_value = "5")]
    pub async_capture_limit: u32,

    /// Milliseconds between concurrency ramp-up steps (1 additional scenario slot per step)
    #[clap(long, allow_hyphen_values = true, default_value = "1000")]
    pub async_capture_ramp_ms: u64,

    /// Enable debug mode
    #[clap(long, action, default_value = "false")]
    pub debug: bool,

    /// Force scenarios into required-login mode for custom jsOnReady scripts
    #[clap(
        long = "requiredLogin",
        visible_alias = "required-login",
        action,
        default_value = "false"
    )]
    pub required_login: bool,

    /// Subcommand to execute
    #[clap(subcommand)]
    pub command: Command,
}
