use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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
    /// Copy from system to shed
    Put(SyncArgs),
    /// Copy from shed to system
    Get(SyncArgs),
    /// Manifest utilities
    Manifest {
        #[command(subcommand)]
        command: ManifestCommand,
    },
}

#[derive(Args)]
pub struct SyncArgs {
    /// Print operations without executing
    #[arg(long)]
    pub dry: bool,

    /// Print detailed output
    #[arg(short, long)]
    pub verbose: bool,

    /// Disable the interactive progress display
    #[arg(long)]
    pub no_progress: bool,

    /// Manifest YAML files
    #[arg(required = true, value_name = "MANIFEST")]
    pub manifests: Vec<PathBuf>,
}

#[derive(Subcommand)]
pub enum ManifestCommand {
    /// Print the JSON Schema for manifest YAML
    Schema,
}

impl Cli {
    pub fn parse_args() -> Self {
        Parser::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_command_first_sync_arguments() {
        let cli = Cli::try_parse_from(["shed", "put", "--dry", "one.yaml", "two.yaml"]).unwrap();
        let Command::Put(args) = cli.command else {
            panic!("expected put");
        };
        assert!(args.dry);
        assert_eq!(
            args.manifests,
            [PathBuf::from("one.yaml"), PathBuf::from("two.yaml")]
        );
    }

    #[test]
    fn rejects_sync_options_before_the_command() {
        assert!(Cli::try_parse_from(["shed", "--dry", "put", "one.yaml"]).is_err());
    }

    #[test]
    fn requires_at_least_one_manifest() {
        assert!(Cli::try_parse_from(["shed", "get"]).is_err());
    }

    #[test]
    fn parses_manifest_schema_command() {
        let cli = Cli::try_parse_from(["shed", "manifest", "schema"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Manifest {
                command: ManifestCommand::Schema
            }
        ));
    }
}
