use clap::Subcommand;

use crate::cli::args::OptimizeArgs;

#[cfg(feature = "local")]
use crate::cli::args::TestArgs;

#[derive(Subcommand)]
pub enum Command {
    /// Snapshot command
    Snapshot,

    /// Optimize images command
    Optimize(OptimizeArgs),

    /// Test command (not implemented yet)
    #[cfg(feature = "local")]
    Test(TestArgs),
}
