use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use walkdir::WalkDir;

use crate::item::ResolvedItem;
use crate::progress::{self, Event, Reporter};

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

pub struct Cancellation {
    count: Arc<AtomicUsize>,
}

impl Cancellation {
    pub fn install() -> Result<Self, crate::error::Error> {
        let count = Arc::new(AtomicUsize::new(0));
        let signal_count = Arc::clone(&count);

        ctrlc::try_set_handler(move || {
            let previous = signal_count.fetch_add(1, Ordering::SeqCst);
            if previous >= 1 {
                std::process::exit(130);
            }
        })
        .map_err(crate::error::Error::CtrlC)?;

        Ok(Self { count })
    }

    pub fn is_cancelled(&self) -> bool {
        self.count.load(Ordering::SeqCst) > 0
    }

    #[cfg(test)]
    pub fn inactive() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[cfg(test)]
    pub fn cancel(&self) {
        self.count.store(1, Ordering::SeqCst);
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

pub fn execute(
    items: &[ResolvedItem],
    direction: Direction,
    dry: bool,
    verbose: bool,
    no_progress: bool,
    cancellation: &Cancellation,
) -> Result<Outcome, crate::error::Error> {
    let started = Instant::now();
    let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
    let mut reporter = progress::reporter(direction, &names, dry, verbose, no_progress);
    Ok(execute_with_reporter(
        items,
        direction,
        dry,
        cancellation,
        reporter.as_mut(),
        started,
    ))
}

fn execute_with_reporter(
    items: &[ResolvedItem],
    direction: Direction,
    dry: bool,
    cancellation: &Cancellation,
    reporter: &mut dyn Reporter,
    started: Instant,
) -> Outcome {
    let mut counters = Counters::new(items.len());

    let plan = match build_plan(items, direction, cancellation, reporter, &mut counters) {
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
            reporter.report(Event::Finish(&summary));
            return outcome_from(&summary);
        }
    };

    reporter.report(Event::PlanFinished {
        total_files: plan.total_files,
    });

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
        reporter.report(Event::Finish(&summary));
        return outcome_from(&summary);
    }

    let mut cancelled = false;

    for (item_index, item_plan) in plan.items.iter().enumerate() {
        if cancellation.is_cancelled() {
            cancelled = true;
            break;
        }

        counters.statuses[item_index] = ItemStatus::Copying;
        reporter.report(Event::ItemStarted { item: item_index });

        let mut item_had_error = item_plan.had_error;
        let mut failed_directories: Vec<PathBuf> = Vec::new();

        for operation in &item_plan.operations {
            if cancellation.is_cancelled() {
                cancelled = true;
                break;
            }

            match operation {
                PlannedOperation::CreateDir { dst } => {
                    if is_blocked(dst, &failed_directories) || dry {
                        continue;
                    }
                    if let Err(error) = fs::create_dir_all(dst) {
                        item_had_error = true;
                        counters.errors += 1;
                        failed_directories.push(dst.clone());
                        let message = format!("creating directory {:?}: {}", dst, error);
                        reporter.report(Event::Error {
                            item: Some(item_index),
                            message: &message,
                        });
                    }
                }
                PlannedOperation::CopyFile {
                    src,
                    dst,
                    logical_path,
                } => {
                    reporter.report(Event::FileStarted {
                        item: item_index,
                        logical_path,
                        src,
                        dst,
                    });

                    let blocked = is_blocked(dst, &failed_directories);
                    let mut copied = false;
                    let mut bytes = 0;

                    if !blocked {
                        if dry {
                            copied = true;
                        } else {
                            match fs::copy(src, dst) {
                                Ok(count) => {
                                    copied = true;
                                    bytes = count;
                                }
                                Err(error) => {
                                    item_had_error = true;
                                    counters.errors += 1;
                                    let message = format!("{:?} -> {:?}: {}", src, dst, error);
                                    reporter.report(Event::Error {
                                        item: Some(item_index),
                                        message: &message,
                                    });
                                }
                            }
                        }
                    } else {
                        item_had_error = true;
                    }

                    counters.processed_files += 1;
                    if copied {
                        counters.copied_files += 1;
                        counters.copied_bytes += bytes;
                    }
                    reporter.report(Event::FileFinished {
                        item: item_index,
                        copied,
                        bytes,
                    });
                }
            }

            if cancellation.is_cancelled() {
                cancelled = true;
                break;
            }
        }

        if cancelled {
            counters.statuses[item_index] = ItemStatus::Cancelled;
            reporter.report(Event::ItemFinished {
                item: item_index,
                status: ItemStatus::Cancelled,
            });
            break;
        }

        let status = if item_had_error {
            ItemStatus::Failed
        } else if !item_plan.had_source && item_plan.missing_sources > 0 {
            ItemStatus::Skipped
        } else if item_plan.missing_sources > 0 {
            ItemStatus::Warning
        } else {
            ItemStatus::Done
        };
        counters.statuses[item_index] = status;
        reporter.report(Event::ItemFinished {
            item: item_index,
            status,
        });
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
    reporter.report(Event::Finish(&summary));

    outcome_from(&summary)
}

fn build_plan(
    items: &[ResolvedItem],
    direction: Direction,
    cancellation: &Cancellation,
    reporter: &mut dyn Reporter,
    counters: &mut Counters,
) -> PlanResult {
    let mut planned_items = Vec::with_capacity(items.len());
    let mut discovered_files = 0;
    let mut scanned_items = 0;

    for (item_index, item) in items.iter().enumerate() {
        if cancellation.is_cancelled() {
            return PlanResult::Cancelled {
                scanned_items,
                discovered_files,
            };
        }

        counters.statuses[item_index] = ItemStatus::Scanning;
        reporter.report(Event::ScanStarted { item: item_index });

        let mut operations = Vec::new();
        let mut total_files = 0;
        let mut missing_sources = 0;
        let mut had_source = false;
        let mut had_error = false;

        for entry in &item.entries {
            if cancellation.is_cancelled() {
                return PlanResult::Cancelled {
                    scanned_items,
                    discovered_files,
                };
            }

            let (src, dst) = endpoints(entry, direction);
            if !src.exists() {
                missing_sources += 1;
                counters.warnings += 1;
                let message = format!("source not found: {:?}", src);
                reporter.report(Event::Warning {
                    item: Some(item_index),
                    message: &message,
                });
                continue;
            }
            had_source = true;

            if src.is_dir() {
                for walked in WalkDir::new(src) {
                    if cancellation.is_cancelled() {
                        return PlanResult::Cancelled {
                            scanned_items,
                            discovered_files,
                        };
                    }

                    match walked {
                        Ok(walked) => {
                            let src_path = walked.path();
                            let relative = src_path.strip_prefix(src).unwrap_or(Path::new(""));
                            let dst_path = dst.join(relative);
                            let logical_path = logical_path(item, &dst_path);
                            reporter.report(Event::ScanPath {
                                item: item_index,
                                logical_path: &logical_path,
                            });

                            if src_path.is_dir() {
                                operations.push(PlannedOperation::CreateDir { dst: dst_path });
                            } else {
                                total_files += 1;
                                discovered_files += 1;
                                operations.push(PlannedOperation::CopyFile {
                                    src: src_path.to_path_buf(),
                                    dst: dst_path,
                                    logical_path,
                                });
                            }
                        }
                        Err(error) => {
                            had_error = true;
                            counters.errors += 1;
                            let message = format!("walking {:?}: {}", src, error);
                            reporter.report(Event::Error {
                                item: Some(item_index),
                                message: &message,
                            });
                        }
                    }

                    if cancellation.is_cancelled() {
                        return PlanResult::Cancelled {
                            scanned_items,
                            discovered_files,
                        };
                    }
                }
            } else {
                if let Some(parent) = dst.parent() {
                    operations.push(PlannedOperation::CreateDir {
                        dst: parent.to_path_buf(),
                    });
                }
                let logical_path = logical_path(item, dst);
                reporter.report(Event::ScanPath {
                    item: item_index,
                    logical_path: &logical_path,
                });
                operations.push(PlannedOperation::CopyFile {
                    src: src.to_path_buf(),
                    dst: dst.to_path_buf(),
                    logical_path,
                });
                total_files += 1;
                discovered_files += 1;
            }
        }

        scanned_items += 1;
        counters.statuses[item_index] = ItemStatus::Ready;
        reporter.report(Event::ScanFinished {
            item: item_index,
            total_files,
        });
        planned_items.push(ItemPlan {
            operations,
            missing_sources,
            had_source,
            had_error,
        });
    }

    PlanResult::Complete(SyncPlan {
        items: planned_items,
        total_files: discovered_files,
    })
}

fn endpoints(entry: &crate::item::ResolvedEntry, direction: Direction) -> (&Path, &Path) {
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
    use crate::item::ResolvedEntry;
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

    fn run(items: &[ResolvedItem], direction: Direction, dry: bool) -> Outcome {
        execute(
            items,
            direction,
            dry,
            false,
            true,
            &Cancellation::inactive(),
        )
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

    #[test]
    fn emits_planning_then_execution_events() {
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
        );

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

    #[test]
    fn cancellation_between_files_stops_without_rollback() {
        struct CancelAfterFirstFile<'a> {
            cancellation: &'a Cancellation,
            files: usize,
            summary: Option<Summary>,
        }

        impl Reporter for CancelAfterFirstFile<'_> {
            fn report(&mut self, event: Event<'_>) {
                match event {
                    Event::FileFinished { .. } => {
                        self.files += 1;
                        self.cancellation.cancel();
                    }
                    Event::Finish(summary) => self.summary = Some(summary.clone()),
                    _ => {}
                }
            }
        }

        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("one.txt"), "one").unwrap();
        fs::write(source.join("two.txt"), "two").unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("tree");
        let item = resolved_item("item", shed_base, vec![(source, destination.clone())]);
        let cancellation = Cancellation::inactive();
        let mut reporter = CancelAfterFirstFile {
            cancellation: &cancellation,
            files: 0,
            summary: None,
        };

        let outcome = execute_with_reporter(
            &[item],
            Direction::Put,
            false,
            &cancellation,
            &mut reporter,
            Instant::now(),
        );

        assert!(outcome.cancelled);
        assert_eq!(reporter.files, 1);
        assert_eq!(reporter.summary.unwrap().copied_files, 1);
        let copied = [destination.join("one.txt"), destination.join("two.txt")]
            .into_iter()
            .filter(|path| path.exists())
            .count();
        assert_eq!(copied, 1);
    }

    #[test]
    fn recursively_copies_files_and_empty_directories() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let empty = source.join("nested/empty");
        fs::create_dir_all(&empty).unwrap();
        fs::write(source.join("root.txt"), "root").unwrap();
        fs::write(source.join("nested/file.txt"), "nested").unwrap();

        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("config");
        let item = resolved_item("item", shed_base, vec![(source, destination.clone())]);

        let outcome = run(&[item], Direction::Put, false);

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

    #[test]
    fn missing_source_is_a_warning_and_does_not_create_a_destination() {
        let temp = TempDir::new().unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("missing.txt");
        let item = resolved_item(
            "item",
            shed_base,
            vec![(temp.path().join("absent.txt"), destination.clone())],
        );

        let outcome = run(&[item], Direction::Put, false);

        assert_eq!(outcome.warnings, 1);
        assert_eq!(outcome.errors, 0);
        assert_eq!(outcome.exit_code(), ExitCode::from(1));
        assert!(!destination.exists());
    }

    #[test]
    fn dry_run_plans_but_does_not_write() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        fs::write(&source, "content").unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("target.txt");
        let item = resolved_item("item", shed_base, vec![(source, destination.clone())]);

        let outcome = run(&[item], Direction::Put, true);

        assert_eq!(outcome.exit_code(), ExitCode::SUCCESS);
        assert!(!destination.exists());
    }

    #[test]
    fn later_entries_keep_last_writer_wins_behavior() {
        let temp = TempDir::new().unwrap();
        let first = temp.path().join("first.txt");
        let second = temp.path().join("second.txt");
        fs::write(&first, "first").unwrap();
        fs::write(&second, "second").unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("same.txt");
        let item = resolved_item(
            "item",
            shed_base,
            vec![(first, destination.clone()), (second, destination.clone())],
        );

        let outcome = run(&[item], Direction::Put, false);

        assert_eq!(outcome.exit_code(), ExitCode::SUCCESS);
        assert_eq!(fs::read_to_string(destination).unwrap(), "second");
    }

    #[test]
    fn get_reverses_the_copy_direction() {
        let temp = TempDir::new().unwrap();
        let shed_base = temp.path().join("shed/item");
        fs::create_dir_all(&shed_base).unwrap();
        let shed_file = shed_base.join("stored.txt");
        let system_file = temp.path().join("system/loaded.txt");
        fs::write(&shed_file, "stored").unwrap();
        let item = resolved_item("item", shed_base, vec![(system_file.clone(), shed_file)]);

        let outcome = run(&[item], Direction::Get, false);

        assert_eq!(outcome.exit_code(), ExitCode::SUCCESS);
        assert_eq!(fs::read_to_string(system_file).unwrap(), "stored");
    }

    #[test]
    fn directory_failure_blocks_descendants_with_one_error() {
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

        let outcome = run(&[item], Direction::Put, false);

        assert_eq!(outcome.errors, 1);
        assert_eq!(outcome.exit_code(), ExitCode::from(2));
    }

    #[test]
    fn cancellation_before_planning_writes_nothing() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        fs::write(&source, "content").unwrap();
        let shed_base = temp.path().join("shed/item");
        let destination = shed_base.join("target.txt");
        let item = resolved_item("item", shed_base, vec![(source, destination.clone())]);
        let cancellation = Cancellation::inactive();
        cancellation.cancel();

        let outcome = execute(&[item], Direction::Put, false, false, true, &cancellation).unwrap();

        assert!(outcome.cancelled);
        assert_eq!(outcome.exit_code(), ExitCode::from(130));
        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn nested_directory_symlinks_are_not_followed() {
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

        let outcome = run(&[item], Direction::Put, false);

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
