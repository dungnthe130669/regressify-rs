use std::{
    env::set_current_dir,
    path::{Path, PathBuf},
    process::exit,
};

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
mod github;
mod helpers;
mod optimizer;
mod snapshot;
mod test;
mod types;
mod vso;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let pause = cli.pause;
    let pause_if_error = cli.pause_if_error;
    let mut exit_code = 1;

    match exec_commands(cli).await {
        Ok(_) => {
            exit_code = 0;
        }
        Err(e) => {
            eprintln!("{} {:?}", "ERROR:".red(), e);
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
    let work_dir: PathBuf = cli.root.normalize()?;

    eprintln!("Setting working directory to {:?}", work_dir);
    set_current_dir(&work_dir).map_err(|e| anyhow!("Failed to set current directory: {}", e))?;
    set_global_vars(&cli, &work_dir)?;

    match cli.command {
        Command::Snapshot => snapshot::snapshot(),
        Command::Optimize(args) => optimizer::optimize(args),
        Command::Test => test::test().await,
    }
}

fn set_global_vars(cli: &Cli, work_dir: &Path) -> Result<()> {
    let normal_path = work_dir.to_path_buf();

    let backstop_dir_name = if let Some(ref site_env) = cli.site_env {
        format!(".backstop__{}", site_env)
    } else {
        ".backstop".to_string()
    };

    let backstop_dir = normal_path.join(&backstop_dir_name);

    common::BACKSTOP_DIR_NAME.set_value(backstop_dir_name)?;
    common::BACKSTOP_DIR.set_value(backstop_dir)?;
    common::ROOT_DIR.set_value(normal_path)?;

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
