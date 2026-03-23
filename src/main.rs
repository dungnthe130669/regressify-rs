use std::{env::set_current_dir, path::PathBuf, process::exit};

use anyhow::{Result, anyhow};
use clap::Parser;
use colored::Colorize;
use reqwest::Proxy;

use crate::{
    cli::{Cli, Command},
    github::is_github,
    helpers::{IOExtensions, OnceLockBoolExtensions, OnceLockExtensions},
    vso::is_azure_devops,
};

mod cli;
mod common;
mod compare;
mod github;
mod helpers;
mod optimizer;
mod snapshot;
mod types;
mod vso;

#[cfg(feature = "local")]
mod test;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let pause = cli.pause;
    let pause_if_error = cli.pause_if_error;
    let mut exit_code = 1;

    match exec_commands(cli).await {
        Ok(_) => {
            exit_code = 0;

            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let msg = format!("[{}] Finished successfully.", now).dimmed();
            println!("{}", msg);
        }
        Err(e) => {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            eprintln!("[{}] {} {:?}", now, "ERROR:".red(), e);
        }
    }

    if pause || (pause_if_error && exit_code != 0) {
        eprintln!(" ");
        eprint!("Press any key to continue...");
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).ok();
    }

    exit(exit_code);
}

async fn exec_commands(cli: Cli) -> Result<()> {
    set_root(&cli)?;
    set_global_vars(&cli)?;

    match cli.command {
        Command::Snapshot(args) => snapshot::snapshot(args),
        Command::Optimize(args) => optimizer::optimize(args),
        Command::Compare(args) => compare::compare(args),
        Command::Approve(args) => compare::approve(args),

        #[cfg(feature = "local")]
        Command::Test(args) => test::exec_test_suites(args).await,

        #[cfg(feature = "local")]
        Command::Ref(args) => test::exec_ref(args).await,
    }
}

fn set_global_vars(cli: &Cli) -> Result<()> {
    let backstop_dir_name = if let Some(ref site_env) = cli.site_env {
        format!(".backstop__{}", site_env)
    } else {
        ".backstop".to_string()
    };

    let work_dir = common::ROOT_DIR.get_required()?;
    let backstop_dir = work_dir.join(&backstop_dir_name);

    common::BACKSTOP_DIR_NAME.set_value(backstop_dir_name)?;
    common::BACKSTOP_DIR.set_value(backstop_dir)?;
    common::DEBUG.set_value(cli.debug)?;

    if let Some(ref proxy) = cli.proxy {
        let proxy = Proxy::all(proxy)?;
        common::PROXY.set_value(proxy)?;
    }

    if cli.no_print {
        common::NO_PRINT.set_bool(true)?;
    }

    if cli.verbose {
        common::VERBOSE.set_bool(true)?;
    }

    if is_github() || is_azure_devops() {
        // Force color output
        colored::control::set_override(true);
    }

    Ok(())
}

fn set_root(cli: &Cli) -> Result<()> {
    let work_dir: PathBuf = cli.root.normalize()?;

    set_current_dir(&work_dir).map_err(|e| anyhow!("Failed to set current directory: {}", e))?;
    common::ROOT_DIR.set_value(work_dir)?;

    Ok(())
}
