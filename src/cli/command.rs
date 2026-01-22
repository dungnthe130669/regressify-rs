use clap::Subcommand;

use crate::cli::args::OptimizeArgs;

#[derive(Subcommand)]
pub enum Command {
    /// Snapshot command
    Snapshot,

    /// Optimize images command
    Optimize(OptimizeArgs),
}
