use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "shed",
    version,
    about = "Sync dotfiles between system and shed"
)]
pub struct Cli {
    /// Path to collection YAML
    #[arg(short, long)]
    pub collection: Option<PathBuf>,

    /// Path to items directory
    #[arg(short, long)]
    pub items_dir: Option<PathBuf>,

    /// Print operations without executing
    #[arg(long)]
    pub dry: bool,

    /// Print detailed output
    #[arg(short, long)]
    pub verbose: bool,

    /// Disable the interactive progress display
    #[arg(long, global = true)]
    pub no_progress: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Copy from system to shed
    Put {
        /// Target directory (default: current directory)
        #[arg(value_name = "DIR")]
        target: Option<PathBuf>,
    },
    /// Copy from shed to system
    Get {
        /// Source directory (default: current directory)
        #[arg(value_name = "DIR")]
        source: Option<PathBuf>,
    },
    /// Configuration utilities
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Print JSON schemas
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
}

#[derive(Subcommand)]
pub enum SchemaCommand {
    /// Print JSON schema for collection YAML
    Collections,
    /// Print JSON schema for item YAML
    Items,
}

impl Cli {
    pub fn parse_args() -> Self {
        Parser::parse()
    }
}
