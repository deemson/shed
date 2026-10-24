use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "shed",
    version = env!("SHED_VERSION"),
    about = "Sync dotfiles between system and shed"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Put(PutGetArgs),
    Get(PutGetArgs),
    Manifest {
        #[command(subcommand)]
        command: ManifestCommand,
    },
}

#[derive(Args)]
pub struct PutGetArgs {
    /// Show what would change without touching any files
    #[arg(long)]
    pub dry: bool,

    /// Print every entry in the report, not just the summary
    #[arg(short, long)]
    pub verbose: bool,

    #[arg(required = true, value_name = "MANIFEST")]
    pub manifests: Vec<PathBuf>,
}

#[derive(Subcommand)]
pub enum ManifestCommand {
    Schema,
}

impl Cli {
    pub fn parse_args() -> Self {
        Parser::parse()
    }
}
