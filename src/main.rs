use std::process::exit;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use regressify::{application, cli::Cli};

#[tokio::main]
async fn main() -> Result<()> {
    regressify::diagnostics::install::install_subscriber();
    let cli = Cli::parse();
    regressify::diagnostics::install::configure_console(
        cli.log_level,
        cli.log_verbose || cli.verbose,
        cli.no_print,
    )?;
    let pause = cli.pause;
    let pause_if_error = cli.pause_if_error;
    let mut exit_code = 1;

    match application::execute(cli).await {
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

    regressify::diagnostics::install::flush();
    exit(exit_code);
}
