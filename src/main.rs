mod cli;
mod collection;
mod error;
mod item;
mod sync;

use std::process::ExitCode;

use cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse_args();
    let result = run(&cli);

    match result {
        Ok(outcome) => outcome.exit_code(),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: &Cli) -> Result<sync::Outcome, error::Error> {
    let items_dir = cli.items_dir.canonicalize().map_err(|e| {
        error::Error::Io(format!("items directory {:?}", cli.items_dir), e)
    })?;

    let collection_path = cli.collection.canonicalize().map_err(|e| {
        error::Error::Io(format!("collection {:?}", cli.collection), e)
    })?;

    let resolved = collection::resolve(&collection_path)?;

    if resolved.abstract_root {
        return Err(error::Error::AbstractCollection(collection_path));
    }

    let items = item::load_all(&resolved.item_refs, &items_dir)?;

    let direction = match cli.command {
        Command::Put => sync::Direction::Put,
        Command::Get => sync::Direction::Get,
    };

    sync::execute(&items, direction, cli.dry, cli.verbose)
}
