//! Shared engine entry point. Each invocation owns one process's run settings.

use std::{env::set_current_dir, path::PathBuf};

use anyhow::{Result, anyhow};
use reqwest::Proxy;

use crate::{
    cli::{Cli, Command},
    common, compare,
    github::is_github,
    helpers::{IOExtensions, OnceLockBoolExtensions, OnceLockExtensions},
    init, optimizer, snapshot, test,
    vso::is_azure_devops,
};

/// Called by the CLI or an isolated desktop worker, never by the GUI thread.
pub async fn execute(cli: Cli) -> Result<()> {
    set_root(&cli)?;
    set_global_vars(&cli)?;
    let phase = match &cli.command {
        Command::Test(_) | Command::Ref(_) => "Validating test suites",
        Command::Compare(_) => "Comparing screenshots",
        Command::Approve(_) => "Approving baselines",
        Command::Snapshot(_) => "Generating report",
        Command::Optimize(_) => "Optimizing images",
        Command::Install => "Installing Chromium",
        Command::Init => "Initializing workspace",
        Command::Version => "Reading version",
    };
    crate::progress::emit(crate::progress::Event::Phase { name: phase.into() });

    match cli.command {
        Command::Init => init::init(),
        Command::Install => {
            let executable = test::fetcher()
                .await
                .map_err(|error| anyhow!("Failed to fetch the browser: {error:?}"))?;
            eprintln!("Browser executable: {}", executable.display());
            Ok(())
        }
        Command::Version => {
            println!(
                "{} ({})",
                env!("CARGO_PKG_VERSION"),
                env!("BINARY_GIT_HASH")
            );
            Ok(())
        }
        Command::Snapshot(args) => snapshot::snapshot(args),
        Command::Optimize(args) => optimizer::optimize(args),
        Command::Compare(args) => compare::compare(args),
        Command::Approve(args) => compare::approve(args),
        Command::Test(args) => test::exec_test_suites(args).await,
        Command::Ref(args) => test::exec_ref(args).await,
    }
}

fn set_global_vars(cli: &Cli) -> Result<()> {
    let work_dir = common::ROOT_DIR.get_required()?;
    let regressify_dir = if let Some(regressify_dir) = &cli.regressify_dir {
        work_dir.join(regressify_dir)
    } else {
        let directory_name = if let Some(site_env) = &cli.site_env {
            format!(".regressify__{site_env}")
        } else {
            ".regressify".to_string()
        };
        work_dir.join(directory_name)
    };

    common::REGRESSIFY_DIR.set_value(regressify_dir)?;
    if let Some(run_id) = &cli.run_id {
        common::RUN_ID.set_value(run_id.clone())?;
    }
    common::DEBUG.set_value(cli.debug)?;
    common::REQUIRED_LOGIN.set_value(cli.required_login)?;
    common::ASYNC_CAPTURE_LIMIT.set_value(cli.async_capture_limit as usize)?;
    common::ASYNC_CAPTURE_RAMP_MS.set_value(cli.async_capture_ramp_ms)?;

    if let Some(proxy) = &cli.proxy {
        common::PROXY.set_value(Proxy::all(proxy)?)?;
    }
    if cli.no_print {
        common::NO_PRINT.set_bool(true)?;
    }
    if cli.verbose {
        common::VERBOSE.set_bool(true)?;
    }
    if is_github() || is_azure_devops() {
        colored::control::set_override(true);
    }
    Ok(())
}

fn set_root(cli: &Cli) -> Result<()> {
    let work_dir: PathBuf = cli.root.normalize()?;
    set_current_dir(&work_dir)
        .map_err(|error| anyhow!("Failed to set current directory: {error}"))?;
    common::ROOT_DIR.set_value(work_dir)?;
    Ok(())
}
