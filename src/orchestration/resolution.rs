use super::model::*;
use crate::manifest;

pub fn outcome(events: &[manifest::Event]) -> PhaseOutcome {
    events
        .iter()
        .filter_map(|event| match event {
            manifest::Event::ResolutionFinished {
                error: Some(error), ..
            } => Some(error_outcome(error)),
            manifest::Event::ResolutionStarted { .. }
            | manifest::Event::ResolutionFinished { error: None, .. }
            | manifest::Event::Done => None,
        })
        .max()
        .unwrap_or(PhaseOutcome::NoErrors)
}

fn error_outcome(error: &manifest::Error) -> PhaseOutcome {
    match error {
        manifest::Error::NotFound | manifest::Error::Io { .. } | manifest::Error::Yaml { .. } => {
            PhaseOutcome::BlockingErrors
        }
        manifest::Error::Cycle { .. } => PhaseOutcome::NonBlockingErrors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io;

    fn events(errors: impl IntoIterator<Item = Option<manifest::Error>>) -> Vec<manifest::Event> {
        let mut events = Vec::new();
        for (position, error) in errors.into_iter().enumerate() {
            events.push(manifest::Event::ResolutionStarted {
                index: vec![position],
            });
            events.push(manifest::Event::ResolutionFinished {
                index: vec![position],
                error,
            });
        }
        events.push(manifest::Event::Done);
        events
    }

    fn yaml_error() -> yaml_serde::Error {
        yaml_serde::from_str::<Vec<String>>("{").unwrap_err()
    }

    #[test]
    fn gives_no_errors_without_errors() {
        assert_eq!(outcome(&events([None, None])), PhaseOutcome::NoErrors);
    }

    #[test]
    fn blocks_the_run_for_an_absent_include() {
        assert_eq!(
            outcome(&events([None, Some(manifest::Error::NotFound)])),
            PhaseOutcome::BlockingErrors
        );
    }

    #[test]
    fn blocks_the_run_for_an_unreadable_manifest() {
        assert_eq!(
            outcome(&events([Some(manifest::Error::Io {
                source: io::Error::from(io::ErrorKind::PermissionDenied),
            })])),
            PhaseOutcome::BlockingErrors
        );
    }

    #[test]
    fn blocks_the_run_for_invalid_yaml() {
        assert_eq!(
            outcome(&events([Some(manifest::Error::Yaml {
                source: yaml_error(),
            })])),
            PhaseOutcome::BlockingErrors
        );
    }

    #[test]
    fn does_not_block_the_run_for_a_cycle() {
        assert_eq!(
            outcome(&events([
                None,
                Some(manifest::Error::Cycle {
                    start_index: vec![0],
                }),
            ])),
            PhaseOutcome::NonBlockingErrors
        );
    }

    #[test]
    fn blocks_the_run_for_a_cycle_and_an_absent_include() {
        assert_eq!(
            outcome(&events([
                Some(manifest::Error::Cycle {
                    start_index: vec![0],
                }),
                Some(manifest::Error::NotFound),
            ])),
            PhaseOutcome::BlockingErrors
        );
    }
}
