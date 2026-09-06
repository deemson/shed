use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "shed", version, about = "Sync dotfiles between system and shed")]
pub struct Cli {
    /// Path to collection YAML
    #[arg(short, long)]
    pub collection: PathBuf,

    /// Path to items directory
    #[arg(short, long)]
    pub items_dir: PathBuf,

    /// Print operations without executing
    #[arg(long)]
    pub dry: bool,

    /// Print detailed output
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Copy from system to shed
    Put,
    /// Copy from shed to system
    Get,
}

impl Cli {
    pub fn parse_args() -> Self {
        Parser::parse()
    }
}
