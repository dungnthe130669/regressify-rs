#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clap::Parser;
use std::process::ExitCode;

#[derive(Parser)]
#[command(about = "Native Regressify QA workbench", version = regressify::desktop::updates::CURRENT_VERSION)]
struct DesktopArgs {
    /// Workspace directory to open on startup
    workspace: Option<std::path::PathBuf>,
    #[arg(long, hide = true)]
    after_install: bool,
}

fn main() -> ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == regressify::desktop::worker::WORKER_FLAG)
    {
        if let Err(error) = regressify::desktop::updates::protect_installation(false) {
            eprintln!("{error:#}");
            return ExitCode::FAILURE;
        }
        return match regressify::desktop::worker::run_stdio() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error:#}");
                ExitCode::FAILURE
            }
        };
    }
    let args = DesktopArgs::parse();
    #[cfg(target_os = "windows")]
    if let Err(error) = regressify::desktop::jump_list::initialize() {
        eprintln!("Cannot initialize Windows shell integration: {error:#}");
    }
    if let Err(error) = regressify::desktop::updates::protect_installation(args.after_install) {
        rfd::MessageDialog::new()
            .set_title("Regressify Desktop")
            .set_description(format!("{error:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
        return ExitCode::FAILURE;
    }
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
                    #[cfg(target_os = "windows")]
                    regressify::desktop::jump_list::record_workspace(&root);
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
