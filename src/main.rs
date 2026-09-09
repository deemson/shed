mod cli;
mod error;
mod manifest;
mod progress;
mod sync;

use std::process::ExitCode;

use cli::{Cli, Command, ManifestCommand, SyncArgs};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse_args();

    if let Command::Manifest {
        command: ManifestCommand::Schema,
    } = &cli.command
    {
        println!(
            "{}",
            serde_json::to_string_pretty(&manifest::schema()).unwrap()
        );
        return ExitCode::SUCCESS;
    }

    let result = match &cli.command {
        Command::Put(args) => run(args, sync::Direction::Put).await,
        Command::Get(args) => run(args, sync::Direction::Get).await,
        Command::Manifest { .. } => unreachable!(),
    };

    match result {
        Ok(outcome) => outcome.exit_code(),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

async fn run(args: &SyncArgs, direction: sync::Direction) -> Result<sync::Outcome, error::Error> {
    let resolved = manifest::resolve(&args.manifests)?;
    let cancellation = sync::Cancellation::install()?;

    sync::execute_with_layout(
        &resolved.items,
        &resolved.report,
        direction,
        args.dry,
        args.verbose,
        args.no_progress,
        &cancellation,
    )
    .await
}
