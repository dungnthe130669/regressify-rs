use clap::Subcommand;

use crate::cli::args::{ApproveArgs, CompareArgs, OptimizeArgs, RefArgs, SnapshotArgs, TestArgs};

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a Regressify test workspace
    Init,

    /// Fetch/install the Chromium browser used by this CLI
    Install,

    /// Print the CLI version
    Version,

    /// Snapshot command
    Snapshot(SnapshotArgs),

    /// Optimize images command
    Optimize(OptimizeArgs),

    /// Run visual regression tests and compare against reference images
    Test(TestArgs),

    /// Capture reference screenshots (runs tests + auto-approves as baseline)
    Ref(RefArgs),

    /// Compare test screenshots against reference images
    Compare(CompareArgs),

    /// Approve latest test screenshots as new reference images
    Approve(ApproveArgs),
}
