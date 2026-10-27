use super::model::*;
use crate::scanning;

pub fn outcome(events: &[scanning::Event]) -> PhaseOutcome {
    events
        .iter()
        .filter_map(|event| match event {
            scanning::Event::NodeScanningFinished {
                error: Some(error), ..
            }
            | scanning::Event::EntryScanningFinished {
                error: Some(error), ..
            } => Some(error_outcome(error)),
            scanning::Event::NodeScanningStarted { .. }
            | scanning::Event::NodeScanningFinished { error: None, .. }
            | scanning::Event::EntryScanningStarted { .. }
            | scanning::Event::EntryScanningFinished { error: None, .. }
            | scanning::Event::Done => None,
        })
        .max()
        .unwrap_or(PhaseOutcome::NoErrors)
}

fn error_outcome(error: &scanning::Error) -> PhaseOutcome {
    match error {
        scanning::Error::MetadataIo { .. }
        | scanning::Error::ReadDirIo { .. }
        | scanning::Error::UnsupportedFileType { .. } => PhaseOutcome::NonBlockingErrors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io;

    fn node_events(error: Option<scanning::Error>) -> Vec<scanning::Event> {
        vec![
            scanning::Event::NodeScanningStarted {
                index: ([0], [0]).into(),
            },
            scanning::Event::NodeScanningFinished {
                index: ([0], [0]).into(),
                error,
            },
            scanning::Event::Done,
        ]
    }

    fn entry_events(error: Option<scanning::Error>) -> Vec<scanning::Event> {
        let index = scanning::Index {
            manifest: vec![0],
            item: vec![0],
            entry: vec![0],
        };
        vec![
            scanning::Event::NodeScanningStarted {
                index: ([0], [0]).into(),
            },
            scanning::Event::EntryScanningStarted {
                index: index.clone(),
            },
            scanning::Event::EntryScanningFinished { index, error },
            scanning::Event::NodeScanningFinished {
                index: ([0], [0]).into(),
                error: None,
            },
            scanning::Event::Done,
        ]
    }

    fn metadata_io(kind: io::ErrorKind) -> scanning::Error {
        scanning::Error::MetadataIo {
            path: "/src".into(),
            source: io::Error::from(kind),
        }
    }

    #[test]
    fn gives_no_errors_without_errors() {
        assert_eq!(outcome(&node_events(None)), PhaseOutcome::NoErrors);
        assert_eq!(outcome(&entry_events(None)), PhaseOutcome::NoErrors);
    }

    #[test]
    fn does_not_block_the_run_for_an_absent_source() {
        assert_eq!(
            outcome(&node_events(Some(metadata_io(io::ErrorKind::NotFound)))),
            PhaseOutcome::NonBlockingErrors
        );
    }

    #[test]
    fn does_not_block_the_run_for_a_denied_source() {
        assert_eq!(
            outcome(&node_events(Some(metadata_io(
                io::ErrorKind::PermissionDenied
            )))),
            PhaseOutcome::NonBlockingErrors
        );
    }

    #[test]
    fn does_not_block_the_run_for_an_unlistable_directory() {
        assert_eq!(
            outcome(&node_events(Some(scanning::Error::ReadDirIo {
                path: "/src".into(),
                source: io::Error::from(io::ErrorKind::PermissionDenied),
            }))),
            PhaseOutcome::NonBlockingErrors
        );
    }

    #[test]
    fn does_not_block_the_run_for_an_unsupported_file_type() {
        assert_eq!(
            outcome(&node_events(Some(scanning::Error::UnsupportedFileType {
                path: "/src".into(),
            }))),
            PhaseOutcome::NonBlockingErrors
        );
    }

    #[test]
    fn does_not_block_the_run_for_an_entry_error() {
        assert_eq!(
            outcome(&entry_events(Some(metadata_io(io::ErrorKind::NotFound)))),
            PhaseOutcome::NonBlockingErrors
        );
    }
}
