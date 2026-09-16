#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clap::Parser;
use std::process::ExitCode;

#[derive(Parser)]
#[command(about = "Native Regressify QA workbench", version)]
struct DesktopArgs {
    /// Workspace directory to open on startup
    workspace: Option<std::path::PathBuf>,
}

fn main() -> ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == regressify::desktop::worker::WORKER_FLAG)
    {
        return match regressify::desktop::worker::run_stdio() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error:#}");
                ExitCode::FAILURE
            }
        };
    }
    regressify::desktop::ui::launch(DesktopArgs::parse().workspace);
    ExitCode::SUCCESS
}
