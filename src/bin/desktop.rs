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
    let args = DesktopArgs::parse();
    let instance = match args.workspace {
        Some(root) => {
            let result = regressify::desktop::workspace::Preferences::path().and_then(|path| {
                regressify::desktop::instance::DesktopInstance::open(
                    &path.with_file_name("instances"),
                    &root,
                )
            });
            match result {
                Ok(regressify::desktop::instance::OpenInstance::Owned(instance)) => Some(instance),
                Ok(regressify::desktop::instance::OpenInstance::Activated) => {
                    return ExitCode::SUCCESS;
                }
                Err(error) => {
                    rfd::MessageDialog::new()
                        .set_title("Regressify Desktop")
                        .set_description(format!("{error:#}"))
                        .set_level(rfd::MessageLevel::Error)
                        .show();
                    return ExitCode::FAILURE;
                }
            }
        }
        None => None,
    };
    regressify::desktop::ui::launch(instance);
    ExitCode::SUCCESS
}
