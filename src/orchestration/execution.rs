use crate::execution;

pub fn has_errors(events: &[execution::Event]) -> bool {
    events.iter().any(|event| match event {
        execution::Event::EnsureFinished { error, .. }
        | execution::Event::ReplaceFinished { error, .. }
        | execution::Event::EntryFinished { error, .. } => error.is_some(),
        execution::Event::EnsureStarted { .. }
        | execution::Event::ReplaceStarted { .. }
        | execution::Event::EntryStarted { .. }
        | execution::Event::Done => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanning;
    use std::{collections::HashSet, io};

    fn replace_io() -> execution::Error {
        execution::Error::ReplaceIo {
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        }
    }

    #[test]
    fn has_no_errors_without_errors() {
        let events = [
            execution::Event::EnsureStarted { dst: "/dst".into() },
            execution::Event::EnsureFinished {
                dst: "/dst".into(),
                error: None,
            },
            execution::Event::ReplaceStarted {
                kind: execution::Kind::File,
                index: ([0], [0]).into(),
                src: "/src/a".into(),
                dst: "/dst/a".into(),
            },
            execution::Event::ReplaceFinished {
                index: ([0], [0]).into(),
                src: "/src/a".into(),
                dst: "/dst/a".into(),
                error: None,
            },
            execution::Event::Done,
        ];

        assert!(!has_errors(&events));
    }

    #[test]
    fn has_errors_for_an_ensure_error() {
        let events = [
            execution::Event::EnsureFinished {
                dst: "/dst".into(),
                error: Some(execution::Error::EnsureDirectoryIo {
                    source: io::Error::from(io::ErrorKind::PermissionDenied),
                    affected: HashSet::from([([0], [0]).into()]),
                }),
            },
            execution::Event::Done,
        ];

        assert!(has_errors(&events));
    }

    #[test]
    fn has_errors_for_a_replace_error() {
        let events = [
            execution::Event::ReplaceFinished {
                index: ([0], [0]).into(),
                src: "/src/a".into(),
                dst: "/dst/a".into(),
                error: Some(replace_io()),
            },
            execution::Event::Done,
        ];

        assert!(has_errors(&events));
    }

    #[test]
    fn has_errors_for_an_entry_error() {
        let events = [
            execution::Event::EntryFinished {
                index: scanning::Index {
                    manifest: vec![0],
                    item: vec![0],
                    entry: vec![0],
                },
                src: "/src/a/b".into(),
                dst: "/dst/a/b".into(),
                error: Some(replace_io()),
            },
            execution::Event::Done,
        ];

        assert!(has_errors(&events));
    }
}
