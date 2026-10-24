mod cli;
use self::cli::{Cli, Command, ManifestCommand, PutGetArgs};
use shed::{manifest, reporting, scanning, traversal, execution};
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
    };

    ExitCode::SUCCESS
}

async fn run(args: &PutGetArgs, direction: traversal::Direction) {
    let resolve_inputs = args.manifests.iter().map(|path| manifest::Input {
        name: path.display().to_string(),
        path: path.clone(),
    });

    let (resolve_sender, mut resolve_receiver) = mpsc::channel(32);
    let resolver = manifest::Resolver::new(resolve_sender);
    let consume_resolve_events = async {
        let mut events = Vec::new();
        while let Some(event) = resolve_receiver.recv().await {
            let done = matches!(event, manifest::Event::Done);
            events.push(event);

            if done {
                break;
            }
        }
        events
    };
    let (manifests, manifest_events) =
        tokio::join!(resolver.resolve(resolve_inputs), consume_resolve_events);

    let plan = traversal::plan(direction.clone(), &manifests);

    let (scan_sender, mut scan_receiver) = mpsc::channel(32);
    let scanner = scanning::Scanner::new(scan_sender);
    let consume_scan_events = async {
        let mut events = Vec::new();
        while let Some(event) = scan_receiver.recv().await {
            let done = matches!(event, scanning::Event::Done);
            events.push(event);

            if done {
                break;
            }
        }
        events
    };

    let mut report = reporting::summary::Report::new(manifests);
    report.resolution(manifest_events);
    report.traversal(direction, &plan);

    let (scan, scan_events) = tokio::join!(scanner.scan(plan.roots), consume_scan_events);

    report.scanning(scan, scan_events);

    println!("{}", report.render(true));

    let (execute_sender, mut execute_receiver) = mpsc::channel(32);
    let executor = execution::Executor::new(true, execute_sender);

    let consume_execute_events = async {
        let mut events = Vec::new();

        while let Some(event) = execute_receiver.recv().await {
            let done = matches!(event, execution::Event::Done);
            events.push(event);

            if done {
                break;
            }
        }

        events
    };

    let ((), execute_events) = tokio::join!(executor.execute(&scan), consume_execute_events);

    dbg!(execute_events);
}
