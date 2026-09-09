use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_walkdir::WalkDir;
use futures::StreamExt;
use futures::stream::{self, FuturesUnordered};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::manifest::{ReportLayout, ResolvedEntry, ResolvedItem};
#[cfg(test)]
use crate::progress::Event;
use crate::progress::{self, OwnedEvent, Reporter};
#[cfg(test)]
use std::fs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Put, // system -> shed
    Get, // shed -> system
}

impl Direction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Put => "put",
            Self::Get => "get",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemStatus {
    #[default]
    Pending,
    Scanning,
    Ready,
    Copying,
    Done,
    Warning,
    Skipped,
    Failed,
    Cancelled,
}

impl ItemStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Scanning => "scanning",
            Self::Ready => "ready",
            Self::Copying => "copying",
            Self::Done => "done",
            Self::Warning => "warning",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

pub struct Outcome {
    pub warnings: usize,
    pub errors: usize,
    pub cancelled: bool,
}

impl Outcome {
    pub fn exit_code(&self) -> ExitCode {
        if self.cancelled {
            ExitCode::from(130)
        } else if self.errors > 0 {
            ExitCode::from(2)
        } else if self.warnings > 0 {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        }
    }
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub direction: Direction,
    pub total_items: usize,
    pub completed_items: usize,
    pub done_items: usize,
    pub warning_items: usize,
    pub skipped_items: usize,
    pub failed_items: usize,
    pub copied_files: usize,
    pub planned_files: usize,
    pub copied_bytes: u64,
    pub warnings: usize,
    pub errors: usize,
    pub elapsed: Duration,
    pub cancelled: bool,
    pub cancelled_during_planning: bool,
    pub scanned_items: usize,
    pub discovered_files: usize,
}

#[derive(Clone)]
pub struct Cancellation {
    token: CancellationToken,
}

impl Cancellation {
    pub fn install() -> Result<Self, crate::error::Error> {
        let token = CancellationToken::new();
        let signal_token = token.clone();

        ctrlc::try_set_handler(move || {
            if signal_token.is_cancelled() {
                std::process::exit(130);
            }
            signal_token.cancel();
        })
        .map_err(crate::error::Error::CtrlC)?;

        Ok(Self { token })
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    #[cfg(test)]
    pub fn inactive() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

    #[cfg(test)]
    pub fn cancel(&self) {
        self.token.cancel();
    }
}

struct SyncPlan {
    items: Vec<ItemPlan>,
    total_files: usize,
}

struct ItemPlan {
    operations: Vec<PlannedOperation>,
    missing_sources: usize,
    had_source: bool,
    had_error: bool,
}

enum PlannedOperation {
    CreateDir {
        dst: PathBuf,
    },
    CopyFile {
        src: PathBuf,
        dst: PathBuf,
        logical_path: PathBuf,
    },
}

struct Counters {
    warnings: usize,
    errors: usize,
    copied_files: usize,
    processed_files: usize,
    copied_bytes: u64,
    statuses: Vec<ItemStatus>,
}

impl Counters {
    fn new(item_count: usize) -> Self {
        Self {
            warnings: 0,
            errors: 0,
            copied_files: 0,
            processed_files: 0,
            copied_bytes: 0,
            statuses: vec![ItemStatus::Pending; item_count],
        }
    }
}

enum PlanResult {
    Complete(SyncPlan),
    Cancelled {
        scanned_items: usize,
        discovered_files: usize,
    },
}

struct ItemScan {
    item: usize,
    plan: ItemPlan,
    warnings: usize,
    errors: usize,
    discovered_files: usize,
    complete: bool,
}

#[derive(Clone)]
struct CopyWork {
    item: usize,
    src: PathBuf,
    dst: PathBuf,
    logical_path: PathBuf,
}

struct CopyResult {
    item: usize,
    copied: bool,
    bytes: u64,
    error: bool,
}

#[cfg(test)]
pub async fn execute(
    items: &[ResolvedItem],
    direction: Direction,
    dry: bool,
    verbose: bool,
    no_progress: bool,
    cancellation: &Cancellation,
) -> Result<Outcome, crate::error::Error> {
    let layout = ReportLayout::flat(items);
    execute_with_layout(
        items,
        &layout,
        direction,
        dry,
        verbose,
        no_progress,
        cancellation,
    )
    .await
}

pub async fn execute_with_layout(
    items: &[ResolvedItem],
    layout: &ReportLayout,
    direction: Direction,
    dry: bool,
    verbose: bool,
    no_progress: bool,
    cancellation: &Cancellation,
) -> Result<Outcome, crate::error::Error> {
    let started = Instant::now();
    let mut reporter = progress::reporter(direction, layout, dry, verbose, no_progress);
    Ok(execute_with_reporter(
        items,
        direction,
        dry,
        cancellation,
        reporter.as_mut(),
        started,
    )
    .await)
}

async fn execute_with_reporter(
    items: &[ResolvedItem],
    direction: Direction,
    dry: bool,
    cancellation: &Cancellation,
    reporter: &mut dyn Reporter,
    started: Instant,
) -> Outcome {
    let (events, mut receiver) = mpsc::channel(256);
    let jobs = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(4)
        .max(1);

    let work = execute_inner(items, direction, dry, cancellation, events, started, jobs);
    let reporting = async {
        while let Some(event) = receiver.recv().await {
            reporter.report(event.borrowed());
        }
    };

    let (outcome, ()) = tokio::join!(work, reporting);
    outcome
}

async fn execute_inner(
    items: &[ResolvedItem],
    direction: Direction,
    dry: bool,
    cancellation: &Cancellation,
    events: mpsc::Sender<OwnedEvent>,
    started: Instant,
    jobs: usize,
) -> Outcome {
    let mut counters = Counters::new(items.len());

    let plan = match build_plan(items, direction, cancellation, &events, &mut counters).await {
        PlanResult::Complete(plan) => plan,
        PlanResult::Cancelled {
            scanned_items,
            discovered_files,
        } => {
            let summary = make_summary(
                direction,
                items.len(),
                &counters,
                0,
                started.elapsed(),
                true,
                true,
                scanned_items,
                discovered_files,
            );
            send_event(&events, OwnedEvent::Finish(summary.clone())).await;
            return outcome_from(&summary);
        }
    };

    send_event(
        &events,
        OwnedEvent::PlanFinished {
            total_files: plan.total_files,
        },
    )
    .await;

    if cancellation.is_cancelled() {
        let summary = make_summary(
            direction,
            items.len(),
            &counters,
            plan.total_files,
            started.elapsed(),
            true,
            false,
            items.len(),
            plan.total_files,
        );
        send_event(&events, OwnedEvent::Finish(summary.clone())).await;
        return outcome_from(&summary);
    }

    let mut item_had_error: Vec<bool> = plan.items.iter().map(|item| item.had_error).collect();
    for item in 0..plan.items.len() {
        counters.statuses[item] = ItemStatus::Copying;
        send_event(&events, OwnedEvent::ItemStarted { item }).await;
    }

    // Directory creation is deliberately serial. Once it is complete, every
    // copy can run independently without racing its parent directory.
    let mut created_directories = HashSet::new();
    let mut failed_directories = Vec::new();
    let mut directories_complete = vec![false; plan.items.len()];
    for (item, item_plan) in plan.items.iter().enumerate() {
        for operation in &item_plan.operations {
            let PlannedOperation::CreateDir { dst } = operation else {
                continue;
            };
            if cancellation.is_cancelled() {
                break;
            }
            if is_blocked(dst, &failed_directories) {
                item_had_error[item] = true;
                continue;
            }
            if !created_directories.insert(dst.clone()) || dry {
                continue;
            }
            if let Err(error) = tokio::fs::create_dir_all(dst).await {
                item_had_error[item] = true;
                counters.errors += 1;
                failed_directories.push(dst.clone());
                send_event(
                    &events,
                    OwnedEvent::Error {
                        item: Some(item),
                        message: format!("creating directory {:?}: {}", dst, error),
                    },
                )
                .await;
            }
        }
        if cancellation.is_cancelled() {
            break;
        }
        directories_complete[item] = true;
    }

    let mut remaining = vec![0usize; plan.items.len()];
    let mut groups: Vec<Vec<CopyWork>> = Vec::new();
    let mut destinations = HashMap::<PathBuf, usize>::new();
    for (item, item_plan) in plan.items.iter().enumerate() {
        for operation in &item_plan.operations {
            let PlannedOperation::CopyFile {
                src,
                dst,
                logical_path,
            } = operation
            else {
                continue;
            };
            remaining[item] += 1;
            let group = match destinations.get(dst) {
                Some(group) => *group,
                None => {
                    let group = groups.len();
                    destinations.insert(dst.clone(), group);
                    groups.push(Vec::new());
                    group
                }
            };
            groups[group].push(CopyWork {
                item,
                src: src.clone(),
                dst: dst.clone(),
                logical_path: logical_path.clone(),
            });
        }
    }

    let mut cancelled = cancellation.is_cancelled();

    if !cancelled {
        // Copies to the same destination remain ordered, preserving the
        // existing last-writer-wins behavior. Independent destinations run in
        // parallel across the global pool.
        let failed_directories: Arc<[PathBuf]> = failed_directories.into();
        let copy_groups = stream::iter(groups.into_iter().map(|group| {
            copy_group(
                group,
                dry,
                failed_directories.clone(),
                cancellation.clone(),
                events.clone(),
            )
        }));
        let mut copy_groups = copy_groups.buffer_unordered(jobs);

        while let Some(results) = copy_groups.next().await {
            for result in results {
                counters.processed_files += 1;
                remaining[result.item] = remaining[result.item].saturating_sub(1);
                if result.copied {
                    counters.copied_files += 1;
                    counters.copied_bytes += result.bytes;
                }
                if result.error {
                    counters.errors += 1;
                    item_had_error[result.item] = true;
                }
            }
        }
        cancelled = cancellation.is_cancelled();
    }

    for (item, item_plan) in plan.items.iter().enumerate() {
        let status = if cancelled && (!directories_complete[item] || remaining[item] > 0) {
            ItemStatus::Cancelled
        } else {
            final_status(item_plan, item_had_error[item])
        };
        counters.statuses[item] = status;
        send_event(&events, OwnedEvent::ItemFinished { item, status }).await;
    }

    let summary = make_summary(
        direction,
        items.len(),
        &counters,
        plan.total_files,
        started.elapsed(),
        cancelled,
        false,
        items.len(),
        plan.total_files,
    );
    send_event(&events, OwnedEvent::Finish(summary.clone())).await;
    outcome_from(&summary)
}

async fn copy_group(
    group: Vec<CopyWork>,
    dry: bool,
    failed_directories: Arc<[PathBuf]>,
    cancellation: Cancellation,
    events: mpsc::Sender<OwnedEvent>,
) -> Vec<CopyResult> {
    let mut results = Vec::with_capacity(group.len());
    for work in group {
        if cancellation.is_cancelled() {
            break;
        }

        send_event(
            &events,
            OwnedEvent::FileStarted {
                item: work.item,
                logical_path: work.logical_path,
                src: work.src.clone(),
                dst: work.dst.clone(),
            },
        )
        .await;

        let blocked = is_blocked(&work.dst, &failed_directories);
        let (copied, bytes, error) = if blocked {
            // The directory-creation pass already reported and counted the
            // root error. Descendant files are skipped without duplicating it.
            (false, 0, false)
        } else if dry {
            (true, 0, false)
        } else {
            match tokio::fs::copy(&work.src, &work.dst).await {
                Ok(bytes) => (true, bytes, false),
                Err(error) => {
                    send_event(
                        &events,
                        OwnedEvent::Error {
                            item: Some(work.item),
                            message: format!("{:?} -> {:?}: {}", work.src, work.dst, error),
                        },
                    )
                    .await;
                    (false, 0, true)
                }
            }
        };

        send_event(
            &events,
            OwnedEvent::FileFinished {
                item: work.item,
                copied,
                bytes,
            },
        )
        .await;
        results.push(CopyResult {
            item: work.item,
            copied,
            bytes,
            error,
        });
    }
    results
}

async fn build_plan(
    items: &[ResolvedItem],
    direction: Direction,
    cancellation: &Cancellation,
    events: &mpsc::Sender<OwnedEvent>,
    counters: &mut Counters,
) -> PlanResult {
    let mut scans = FuturesUnordered::new();
    for (item, resolved) in items.iter().enumerate() {
        scans.push(scan_item(
            item,
            resolved,
            direction,
            cancellation.clone(),
            events.clone(),
        ));
    }

    let mut planned_items: Vec<Option<ItemPlan>> = (0..items.len()).map(|_| None).collect();
    let mut discovered_files = 0;
    let mut scanned_items = 0;
    let mut interrupted = false;

    while let Some(scan) = scans.next().await {
        counters.warnings += scan.warnings;
        counters.errors += scan.errors;
        discovered_files += scan.discovered_files;
        if scan.complete {
            scanned_items += 1;
            counters.statuses[scan.item] = ItemStatus::Ready;
            planned_items[scan.item] = Some(scan.plan);
        } else {
            interrupted = true;
        }
    }

    if interrupted {
        return PlanResult::Cancelled {
            scanned_items,
            discovered_files,
        };
    }

    PlanResult::Complete(SyncPlan {
        items: planned_items
            .into_iter()
            .map(|item| item.expect("completed scan must produce a plan"))
            .collect(),
        total_files: discovered_files,
    })
}

async fn scan_item(
    item_index: usize,
    item: &ResolvedItem,
    direction: Direction,
    cancellation: Cancellation,
    events: mpsc::Sender<OwnedEvent>,
) -> ItemScan {
    let mut operations = Vec::new();
    let mut total_files = 0;
    let mut missing_sources = 0;
    let mut had_source = false;
    let mut had_error = false;
    let mut warnings = 0;
    let mut errors = 0;

    if cancellation.is_cancelled() {
        return item_scan(
            item_index,
            operations,
            missing_sources,
            had_source,
            had_error,
            warnings,
            errors,
            total_files,
            false,
        );
    }
    send_event(&events, OwnedEvent::ScanStarted { item: item_index }).await;

    for entry in &item.entries {
        if cancellation.is_cancelled() {
            return item_scan(
                item_index,
                operations,
                missing_sources,
                had_source,
                had_error,
                warnings,
                errors,
                total_files,
                false,
            );
        }

        let (src, dst) = endpoints(entry, direction);
        let metadata = match tokio::fs::metadata(src).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing_sources += 1;
                warnings += 1;
                send_event(
                    &events,
                    OwnedEvent::Warning {
                        item: Some(item_index),
                        message: format!("source not found: {:?}", src),
                    },
                )
                .await;
                continue;
            }
            Err(error) => {
                had_error = true;
                errors += 1;
                send_event(
                    &events,
                    OwnedEvent::Error {
                        item: Some(item_index),
                        message: format!("reading source {:?}: {}", src, error),
                    },
                )
                .await;
                continue;
            }
        };
        had_source = true;

        if metadata.is_dir() {
            let root_logical_path = logical_path(item, dst);
            send_event(
                &events,
                OwnedEvent::ScanPath {
                    item: item_index,
                    logical_path: root_logical_path,
                },
            )
            .await;
            operations.push(PlannedOperation::CreateDir {
                dst: dst.to_path_buf(),
            });

            let mut entries = WalkDir::new(src);
            loop {
                let walked = tokio::select! {
                    _ = cancellation.token.cancelled() => {
                        return item_scan(
                            item_index,
                            operations,
                            missing_sources,
                            had_source,
                            had_error,
                            warnings,
                            errors,
                            total_files,
                            false,
                        );
                    }
                    walked = entries.next() => walked,
                };
                let Some(walked) = walked else {
                    break;
                };

                match walked {
                    Ok(walked) => {
                        let src_path = walked.path();
                        let relative = src_path.strip_prefix(src).unwrap_or(Path::new(""));
                        let dst_path = dst.join(relative);
                        let logical_path = logical_path(item, &dst_path);
                        send_event(
                            &events,
                            OwnedEvent::ScanPath {
                                item: item_index,
                                logical_path: logical_path.clone(),
                            },
                        )
                        .await;

                        // metadata follows a directory symlink for compatibility
                        // with the old walker, while async-walkdir itself does not
                        // recurse through that symlink.
                        let is_dir = tokio::fs::metadata(&src_path)
                            .await
                            .is_ok_and(|metadata| metadata.is_dir());
                        if is_dir {
                            operations.push(PlannedOperation::CreateDir { dst: dst_path });
                        } else {
                            total_files += 1;
                            operations.push(PlannedOperation::CopyFile {
                                src: src_path,
                                dst: dst_path,
                                logical_path,
                            });
                        }
                    }
                    Err(error) => {
                        had_error = true;
                        errors += 1;
                        send_event(
                            &events,
                            OwnedEvent::Error {
                                item: Some(item_index),
                                message: format!("walking {:?}: {}", src, error),
                            },
                        )
                        .await;
                    }
                }
            }
        } else {
            if let Some(parent) = dst.parent() {
                operations.push(PlannedOperation::CreateDir {
                    dst: parent.to_path_buf(),
                });
            }
            let logical_path = logical_path(item, dst);
            send_event(
                &events,
                OwnedEvent::ScanPath {
                    item: item_index,
                    logical_path: logical_path.clone(),
                },
            )
            .await;
            operations.push(PlannedOperation::CopyFile {
                src: src.to_path_buf(),
                dst: dst.to_path_buf(),
                logical_path,
            });
            total_files += 1;
        }
    }

    send_event(
        &events,
        OwnedEvent::ScanFinished {
            item: item_index,
            total_files,
        },
    )
    .await;
    item_scan(
        item_index,
        operations,
        missing_sources,
        had_source,
        had_error,
        warnings,
        errors,
        total_files,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn item_scan(
    item: usize,
    operations: Vec<PlannedOperation>,
    missing_sources: usize,
    had_source: bool,
    had_error: bool,
    warnings: usize,
    errors: usize,
    discovered_files: usize,
    complete: bool,
) -> ItemScan {
    ItemScan {
        item,
        plan: ItemPlan {
            operations,
            missing_sources,
            had_source,
            had_error,
        },
        warnings,
        errors,
        discovered_files,
        complete,
    }
}

async fn send_event(events: &mpsc::Sender<OwnedEvent>, event: OwnedEvent) {
    let _ = events.send(event).await;
}

fn final_status(plan: &ItemPlan, had_error: bool) -> ItemStatus {
    if had_error {
        ItemStatus::Failed
    } else if !plan.had_source && plan.missing_sources > 0 {
        ItemStatus::Skipped
    } else if plan.missing_sources > 0 {
        ItemStatus::Warning
    } else {
        ItemStatus::Done
    }
}

fn endpoints(entry: &ResolvedEntry, direction: Direction) -> (&Path, &Path) {
    match direction {
        Direction::Put => (&entry.system, &entry.shed),
        Direction::Get => (&entry.shed, &entry.system),
    }
}

fn logical_path(item: &ResolvedItem, shed_path: &Path) -> PathBuf {
    let relative = shed_path.strip_prefix(&item.shed_base).unwrap_or(shed_path);
    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative.to_path_buf()
    }
}

fn is_blocked(path: &Path, failed_directories: &[PathBuf]) -> bool {
    failed_directories
        .iter()
        .any(|failed| path.starts_with(failed))
}

#[allow(clippy::too_many_arguments)]
fn make_summary(
    direction: Direction,
    total_items: usize,
    counters: &Counters,
    planned_files: usize,
    elapsed: Duration,
    cancelled: bool,
    cancelled_during_planning: bool,
    scanned_items: usize,
    discovered_files: usize,
) -> Summary {
    Summary {
        direction,
        total_items,
        completed_items: counters
            .statuses
            .iter()
            .filter(|status| {
                matches!(
                    status,
                    ItemStatus::Done
                        | ItemStatus::Warning
                        | ItemStatus::Skipped
                        | ItemStatus::Failed
                )
            })
            .count(),
        done_items: count_status(&counters.statuses, ItemStatus::Done),
        warning_items: count_status(&counters.statuses, ItemStatus::Warning),
        skipped_items: count_status(&counters.statuses, ItemStatus::Skipped),
        failed_items: count_status(&counters.statuses, ItemStatus::Failed),
        copied_files: counters.copied_files,
        planned_files,
        copied_bytes: counters.copied_bytes,
        warnings: counters.warnings,
        errors: counters.errors,
        elapsed,
        cancelled,
        cancelled_during_planning,
        scanned_items,
        discovered_files,
    }
}

fn count_status(statuses: &[ItemStatus], wanted: ItemStatus) -> usize {
    statuses.iter().filter(|status| **status == wanted).count()
}

fn outcome_from(summary: &Summary) -> Outcome {
    Outcome {
        warnings: summary.warnings,
        errors: summary.errors,
        cancelled: summary.cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn resolved_item(
        name: &str,
        shed_base: PathBuf,
        entries: Vec<(PathBuf, PathBuf)>,
    ) -> ResolvedItem {
        ResolvedItem {
            name: name.to_string(),
            shed_base,
            entries: entries
                .into_iter()
                .map(|(system, shed)| ResolvedEntry { system, shed })
                .collect(),
        }
    }

    async fn run(items: &[ResolvedItem], direction: Direction, dry: bool) -> Outcome {
        execute(
            items,
            direction,
            dry,
            false,
            true,
            &Cancellation::inactive(),
        )
        .await
        .unwrap()
    }

    #[derive(Default)]
    struct RecordingReporter {
        events: Vec<&'static str>,
        summary: Option<Summary>,
    }

    impl Reporter for RecordingReporter {
        fn report(&mut self, event: Event<'_>) {
            let name = match event {
                Event::ScanStarted { .. } => "scan-started",
                Event::ScanPath { .. } => "scan-path",
                Event::ScanFinished { .. } => "scan-finished",
                Event::PlanFinished { .. } => "plan-finished",
                Event::ItemStarted { .. } => "item-started",
                Event::FileStarted { .. } => "file-started",
                Event::FileFinished { .. } => "file-finished",
                Event::ItemFinished { .. } => "item-finished",
                Event::Warning { .. } => "warning",
                Event::Error { .. } => "error",
                Event::Finish(summary) => {
                    self.summary = Some(summary.clone());
                    "finish"
                }
            };
            self.events.push(name);
        }
    }

    #[tokio::test]
    async fn emits_planning_then_execution_events() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        fs::write(&source, "content").unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("target.txt");
        let item = resolved_item("item", shed_base, vec![(source, destination)]);
        let cancellation = Cancellation::inactive();
        let mut reporter = RecordingReporter::default();

        let outcome = execute_with_reporter(
            &[item],
            Direction::Put,
            false,
            &cancellation,
            &mut reporter,
            Instant::now(),
        )
        .await;

        assert_eq!(outcome.exit_code(), ExitCode::SUCCESS);
        assert_eq!(
            reporter.events,
            [
                "scan-started",
                "scan-path",
                "scan-finished",
                "plan-finished",
                "item-started",
                "file-started",
                "file-finished",
                "item-finished",
                "finish",
            ]
        );
        let summary = reporter.summary.unwrap();
        assert_eq!(summary.copied_files, 1);
        assert_eq!(summary.planned_files, 1);
        assert_eq!(summary.done_items, 1);
    }

    #[tokio::test]
    async fn recursively_copies_files_and_empty_directories() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let empty = source.join("nested/empty");
        fs::create_dir_all(&empty).unwrap();
        fs::write(source.join("root.txt"), "root").unwrap();
        fs::write(source.join("nested/file.txt"), "nested").unwrap();

        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("config");
        let item = resolved_item("item", shed_base, vec![(source, destination.clone())]);

        let outcome = run(&[item], Direction::Put, false).await;

        assert_eq!(outcome.exit_code(), ExitCode::SUCCESS);
        assert_eq!(
            fs::read_to_string(destination.join("root.txt")).unwrap(),
            "root"
        );
        assert_eq!(
            fs::read_to_string(destination.join("nested/file.txt")).unwrap(),
            "nested"
        );
        assert!(destination.join("nested/empty").is_dir());
    }

    #[tokio::test]
    async fn missing_source_is_a_warning_and_does_not_create_a_destination() {
        let temp = TempDir::new().unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("missing.txt");
        let item = resolved_item(
            "item",
            shed_base,
            vec![(temp.path().join("absent.txt"), destination.clone())],
        );

        let outcome = run(&[item], Direction::Put, false).await;

        assert_eq!(outcome.warnings, 1);
        assert_eq!(outcome.errors, 0);
        assert_eq!(outcome.exit_code(), ExitCode::from(1));
        assert!(!destination.exists());
    }

    #[tokio::test]
    async fn dry_run_plans_but_does_not_write() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        fs::write(&source, "content").unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("target.txt");
        let item = resolved_item("item", shed_base, vec![(source, destination.clone())]);

        let outcome = run(&[item], Direction::Put, true).await;

        assert_eq!(outcome.exit_code(), ExitCode::SUCCESS);
        assert!(!destination.exists());
    }

    #[tokio::test]
    async fn get_reverses_the_copy_direction() {
        let temp = TempDir::new().unwrap();
        let shed_base = temp.path().join("shed/item");
        fs::create_dir_all(&shed_base).unwrap();
        let shed_file = shed_base.join("stored.txt");
        let system_file = temp.path().join("system/loaded.txt");
        fs::write(&shed_file, "stored").unwrap();
        let item = resolved_item("item", shed_base, vec![(system_file.clone(), shed_file)]);

        let outcome = run(&[item], Direction::Get, false).await;

        assert_eq!(outcome.exit_code(), ExitCode::SUCCESS);
        assert_eq!(fs::read_to_string(system_file).unwrap(), "stored");
    }

    #[tokio::test]
    async fn directory_failure_blocks_descendants_with_one_error() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("one.txt"), "one").unwrap();
        fs::write(source.join("two.txt"), "two").unwrap();

        let blocker = temp.path().join("blocker");
        fs::write(&blocker, "not a directory").unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = blocker.join("target");
        let item = resolved_item("item", shed_base, vec![(source, destination)]);

        let outcome = run(&[item], Direction::Put, false).await;

        assert_eq!(outcome.errors, 1);
        assert_eq!(outcome.exit_code(), ExitCode::from(2));
    }

    #[tokio::test]
    async fn cancellation_before_planning_writes_nothing() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        fs::write(&source, "content").unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("target.txt");
        let item = resolved_item("item", shed_base, vec![(source, destination.clone())]);
        let cancellation = Cancellation::inactive();
        cancellation.cancel();

        let outcome = execute(&[item], Direction::Put, false, false, true, &cancellation)
            .await
            .unwrap();

        assert!(outcome.cancelled);
        assert_eq!(outcome.exit_code(), ExitCode::from(130));
        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nested_directory_symlinks_are_not_followed() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let external = temp.path().join("external");
        fs::create_dir_all(&external).unwrap();
        fs::write(external.join("outside.txt"), "outside").unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        symlink(&external, source.join("linked")).unwrap();

        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("tree");
        let item = resolved_item("item", shed_base, vec![(source, destination.clone())]);

        let outcome = run(&[item], Direction::Put, false).await;

        assert_eq!(outcome.exit_code(), ExitCode::SUCCESS);
        assert!(destination.join("linked").is_dir());
        assert!(!destination.join("linked/outside.txt").exists());
    }

    #[test]
    fn cancellation_exit_code_takes_precedence() {
        let outcome = Outcome {
            warnings: 1,
            errors: 1,
            cancelled: true,
        };
        assert_eq!(outcome.exit_code(), ExitCode::from(130));
    }

    #[test]
    fn clean_warning_and_error_exit_codes_are_distinct() {
        assert_eq!(
            Outcome {
                warnings: 0,
                errors: 0,
                cancelled: false,
            }
            .exit_code(),
            ExitCode::SUCCESS
        );
        assert_eq!(
            Outcome {
                warnings: 1,
                errors: 0,
                cancelled: false,
            }
            .exit_code(),
            ExitCode::from(1)
        );
        assert_eq!(
            Outcome {
                warnings: 0,
                errors: 1,
                cancelled: false,
            }
            .exit_code(),
            ExitCode::from(2)
        );
    }
}
