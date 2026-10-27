mod cli;
use self::cli::{Cli, Command, ManifestCommand, PutGetArgs};
use shed::{execution, manifest, orchestration, reporting, scanning, traversal};
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

    let resolution_outcome = orchestration::resolution::outcome(&resolve_events);

    if resolution_outcome == orchestration::PhaseOutcome::BlockingErrors {
        let mut report = reporting::summary::Report::new(manifests);
        report.resolution(resolve_events);
        return bail(report, args.verbose);
    }

    let plan = traversal::plan(direction.clone(), &manifests);
    let traversal_outcome = orchestration::traversal::outcome(&plan.errors);

    let mut report = reporting::summary::Report::new(manifests);
    report.resolution(resolve_events);
    report.traversal(direction, &plan);

    if traversal_outcome == orchestration::PhaseOutcome::BlockingErrors {
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

    let scanning_outcome = orchestration::scanning::outcome(&scan_events);
    report.scanning(&scan, scan_events);

    if scanning_outcome == orchestration::PhaseOutcome::BlockingErrors {
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

    let execution_has_errors = orchestration::execution::has_errors(&execute_events);
    report.execution(execute_events);

    let outcome = resolution_outcome
        .max(traversal_outcome)
        .max(scanning_outcome);

    if outcome != orchestration::PhaseOutcome::NoErrors || execution_has_errors {
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

fn bail(report: reporting::summary::Report, verbose: bool) -> ExitCode {
    println!("{}", report.render(verbose));
    ExitCode::FAILURE
}
