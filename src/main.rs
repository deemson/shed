mod cli;
use self::cli::{Cli, Command, ManifestCommand, PutGetArgs};
use shed::{execution, manifest, reporting, scanning, traversal};
use std::process::ExitCode;
use tokio::sync::mpsc;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse_args();

    if let Command::Manifest {
        command: ManifestCommand::Schema,
    } = &cli.command
    {
        println!(
            "{}",
            serde_json::to_string_pretty(&manifest::yaml::schema()).unwrap()
        );
        return ExitCode::SUCCESS;
    }

    match &cli.command {
        Command::Put(args) => run(args, traversal::Direction::Put).await,
        Command::Get(args) => run(args, traversal::Direction::Get).await,
        Command::Manifest { .. } => unreachable!(),
    }
}

async fn run(args: &PutGetArgs, direction: traversal::Direction) -> ExitCode {
    let resolve_inputs = args.manifests.iter().map(|path| manifest::Input {
        name: path.display().to_string(),
        path: path.clone(),
    });

    let (resolve_sender, resolve_receiver) = mpsc::channel(32);
    let resolver = manifest::Resolver::new(resolve_sender);
    let (manifests, resolve_events) = tokio::join!(
        resolver.resolve(resolve_inputs),
        collect_events(resolve_receiver, |event| {
            matches!(event, manifest::Event::Done)
        }),
    );

    if resolution_failed(&resolve_events) {
        let mut report = reporting::summary::Report::new(manifests);
        report.resolution(resolve_events);
        return bail(report, args.verbose);
    }

    let plan = traversal::plan(direction.clone(), &manifests);

    let mut report = reporting::summary::Report::new(manifests);
    report.resolution(resolve_events);
    report.traversal(direction, &plan);

    if !plan.errors.is_empty() {
        return bail(report, args.verbose);
    }

    let (scan_sender, scan_receiver) = mpsc::channel(32);
    let scanner = scanning::Scanner::new(scan_sender);
    let (scan, scan_events) = tokio::join!(
        scanner.scan(plan.roots),
        collect_events(scan_receiver, |event| {
            matches!(event, scanning::Event::Done)
        }),
    );

    let scan_failed = scanning_failed(&scan_events);
    report.scanning(&scan, scan_events);

    if scan_failed {
        return bail(report, args.verbose);
    }

    let (execute_sender, execute_receiver) = mpsc::channel(32);
    let executor = execution::Executor::new(args.dry, execute_sender);
    let ((), execute_events) = tokio::join!(
        executor.execute(&scan),
        collect_events(execute_receiver, |event| {
            matches!(event, execution::Event::Done)
        }),
    );

    let execute_failed = execution_failed(&execute_events);
    report.execution(execute_events);

    if execute_failed {
        return bail(report, args.verbose);
    }

    println!("{}", report.render(args.verbose));
    ExitCode::SUCCESS
}

async fn collect_events<E>(
    mut receiver: mpsc::Receiver<E>,
    is_done: impl Fn(&E) -> bool,
) -> Vec<E> {
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        let done = is_done(&event);
        events.push(event);

        if done {
            break;
        }
    }
    events
}

fn resolution_failed(events: &[manifest::Event]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            manifest::Event::ResolutionFinished { error: Some(_), .. }
        )
    })
}

fn scanning_failed(events: &[scanning::Event]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            scanning::Event::NodeScanningFinished { error: Some(_), .. }
                | scanning::Event::EntryScanningFinished { error: Some(_), .. }
        )
    })
}

fn execution_failed(events: &[execution::Event]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            execution::Event::EnsureFinished { error: Some(_), .. }
                | execution::Event::ReplaceFinished { error: Some(_), .. }
                | execution::Event::EntryFinished { error: Some(_), .. }
        )
    })
}

fn bail(report: reporting::summary::Report, verbose: bool) -> ExitCode {
    println!("{}", report.render(verbose));
    ExitCode::FAILURE
}
