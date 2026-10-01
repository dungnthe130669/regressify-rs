use clap::Parser;
use std::path::PathBuf;

use super::args::{positive_u32_parser, run_id_parser, site_env_parser};

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

    /// Diagnostic journal segment size in bytes (env: REGRESSIFY_DIAGNOSTIC_CHUNK_SIZE_BYTES; default: 524288)
    #[clap(long)]
    pub diagnostic_chunk_size_bytes: Option<usize>,

    /// Per-operation diagnostic disk budget in bytes (env: REGRESSIFY_DIAGNOSTIC_DISK_BUDGET_BYTES; default: 134217728)
    #[clap(long)]
    pub diagnostic_disk_budget_bytes: Option<u64>,

    /// Site environment
    #[clap(long, allow_hyphen_values = true, value_parser = site_env_parser)]
    pub site_env: Option<String>,

    /// Directory to use as regressify directory (overrides site_env)
    #[clap(long, allow_hyphen_values = true)]
    pub regressify_dir: Option<PathBuf>,

    /// Explicit output run ID shared by all selected suites
    #[clap(long, allow_hyphen_values = true, value_parser = run_id_parser)]
    pub run_id: Option<String>,

    /// Number of test scenarios to run in parallel
    #[clap(
        long,
        allow_hyphen_values = true,
        default_value = "5",
        value_parser = positive_u32_parser
    )]
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

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::Cli;

    #[test]
    fn cli_rejects_zero_global_capture_limit() {
        let result = Cli::try_parse_from(["regressify", "--async-capture-limit", "0", "test"]);

        assert!(
            matches!(result, Err(error) if error.to_string().contains("Value must be greater than zero"))
        );
    }

    #[test]
    fn cli_rejects_unsafe_run_id() {
        let result = Cli::try_parse_from(["regressify", "--run-id", "../unsafe", "test"]);

        assert!(matches!(result, Err(error) if error.to_string().contains("Run ID must contain")));
    }

    #[test]
    fn cli_rejects_unsafe_site_environment() {
        let result = Cli::try_parse_from(["regressify", "--site-env", "../unsafe", "test"]);

        assert!(
            matches!(result, Err(error) if error.to_string().contains("Site environment must contain"))
        );
    }
}
