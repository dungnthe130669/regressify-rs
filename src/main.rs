use std::{env::set_current_dir, path::PathBuf, process::exit};

use anyhow::{Result, anyhow};
use clap::Parser;
use colored::Colorize;
use reqwest::Proxy;

use crate::{
    cli::{Cli, Command},
    helpers::{IOExtensions, OnceLockBoolExtensions, OnceLockExtensions},
};

mod cli;
mod common;
mod helpers;
mod optimizer;
mod snapshot;
mod types;

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

    set_current_dir(&work_dir).map_err(|e| anyhow!("Failed to set current directory: {}", e))?;
    set_global_vars(&cli)?;

    match cli.command {
        Command::Report => snapshot::snapshots(vec![]),
        Command::Optimize => optimizer::optimize(),
    }
}

fn set_global_vars(cli: &Cli) -> Result<()> {
    let normal_path = cli.root.normalize()?;
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

    Ok(())
}
