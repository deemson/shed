mod cli;
mod collection;
mod error;
mod item;
mod progress;
mod sync;

use std::process::ExitCode;

use cli::{Cli, Command, ConfigCommand, SchemaCommand};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse_args();

    if let Command::Config {
        command: ConfigCommand::Schema { command },
    } = &cli.command
    {
        let schema = match command {
            SchemaCommand::Collections => collection::schema(),
            SchemaCommand::Items => item::schema(),
        };
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        return ExitCode::SUCCESS;
    }

    let result = run(&cli).await;

    match result {
        Ok(outcome) => outcome.exit_code(),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

async fn run(cli: &Cli) -> Result<sync::Outcome, error::Error> {
    let collection_arg = cli.collection.as_ref().ok_or_else(|| {
        error::Error::MissingArg("--collection is required for put/get".to_string())
    })?;

    let collection_path = collection_arg
        .canonicalize()
        .map_err(|e| error::Error::Io(format!("collection {:?}", collection_arg), e))?;

    // Config resolution: relative to collection file, or explicit --items-dir
    let config_dir = match &cli.items_dir {
        Some(dir) => dir
            .canonicalize()
            .map_err(|e| error::Error::Io(format!("items directory {:?}", dir), e))?,
        None => collection_path
            .parent()
            .ok_or_else(|| {
                error::Error::MissingArg(
                    "cannot determine config directory from collection path".to_string(),
                )
            })?
            .to_path_buf(),
    };

    let cwd = std::env::current_dir()
        .map_err(|e| error::Error::Io("current directory".to_string(), e))?;

    // Determine sync directory and direction from command
    let (direction, sync_dir) = match &cli.command {
        Command::Put { target } => {
            let dir = match target {
                Some(d) => d
                    .canonicalize()
                    .map_err(|e| error::Error::Io(format!("target directory {:?}", d), e))?,
                None => cwd,
            };
            (sync::Direction::Put, dir)
        }
        Command::Get { source } => {
            let dir = match source {
                Some(d) => d
                    .canonicalize()
                    .map_err(|e| error::Error::Io(format!("source directory {:?}", d), e))?,
                None => cwd,
            };
            (sync::Direction::Get, dir)
        }
        Command::Config { .. } => unreachable!(),
    };

    let resolved = collection::resolve(&collection_path, &config_dir)?;

    if resolved.abstract_root {
        return Err(error::Error::AbstractCollection(collection_path));
    }

    let items = item::load_all(&resolved.item_refs, &config_dir, &sync_dir)?;
    let cancellation = sync::Cancellation::install()?;

    sync::execute(
        &items,
        direction,
        cli.dry,
        cli.verbose,
        cli.no_progress,
        &cancellation,
    )
    .await
}
