use clap::Subcommand;

use crate::cli::args::{OptimizeArgs, TestArgs};

#[derive(Subcommand)]
pub enum Command {
    /// Snapshot command
    Snapshot,

    /// Optimize images command
    Optimize(OptimizeArgs),

    /// Test command (not implemented yet)
    Test(TestArgs),
}
