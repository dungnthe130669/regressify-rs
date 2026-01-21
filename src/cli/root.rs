use clap::Parser;
use std::path::PathBuf;

use super::Command;

#[derive(Parser)]
#[clap(version, author, about)]
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

    /// Subcommand to execute
    #[clap(subcommand)]
    pub command: Command,
}
