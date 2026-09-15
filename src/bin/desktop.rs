#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clap::Parser;

#[derive(Parser)]
#[command(about = "Native Regressify QA workbench", version)]
struct DesktopArgs {
    /// Workspace directory to open on startup
    workspace: Option<std::path::PathBuf>,
}

fn main() {
    regressify::desktop::ui::launch(DesktopArgs::parse().workspace);
}
