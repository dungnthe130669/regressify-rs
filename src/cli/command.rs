use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    /// Report command
    Report,
}
