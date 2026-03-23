use clap::Subcommand;

use crate::cli::args::{ApproveArgs, CompareArgs, OptimizeArgs, SnapshotArgs};

#[cfg(feature = "local")]
use crate::cli::args::TestArgs;

#[derive(Subcommand)]
pub enum Command {
    /// Snapshot command
    Snapshot(SnapshotArgs),

    /// Optimize images command
    Optimize(OptimizeArgs),

    /// Test command (not implemented yet)
    #[cfg(feature = "local")]
    Test(TestArgs),

    /// Compare test screenshots against reference images
    Compare(CompareArgs),

    /// Approve latest test screenshots as new reference images
    Approve(ApproveArgs),
}
