use clap::Subcommand;

use crate::cli::args::OptimizeArgs;

#[derive(Subcommand)]
pub enum Command {
    /// Report command
    Report,

    /// Optimize images command
    Optimize(OptimizeArgs),
}
