use clap::Subcommand;

use crate::cli::args::{ApproveArgs, CompareArgs, OptimizeArgs, SnapshotArgs};

#[cfg(feature = "local")]
use crate::cli::args::{RefArgs, TestArgs};

#[derive(Subcommand)]
pub enum Command {
    /// Snapshot command
    Snapshot(SnapshotArgs),

    /// Optimize images command
    Optimize(OptimizeArgs),

    /// Run visual regression tests and compare against reference images
    #[cfg(feature = "local")]
    Test(TestArgs),

    /// Capture reference screenshots (runs tests + auto-approves as baseline)
    #[cfg(feature = "local")]
    Ref(RefArgs),

    /// Compare test screenshots against reference images
    Compare(CompareArgs),

    /// Approve latest test screenshots as new reference images
    Approve(ApproveArgs),
}
