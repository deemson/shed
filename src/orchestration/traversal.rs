use super::model::*;
use crate::traversal;

pub fn outcome(errors: &[traversal::Error]) -> PhaseOutcome {
    errors
        .iter()
        .map(error_outcome)
        .max()
        .unwrap_or(PhaseOutcome::NoErrors)
}

fn error_outcome(error: &traversal::Error) -> PhaseOutcome {
    match error {
        traversal::Error::Expansion { .. }
        | traversal::Error::NonRootPathShedBothAbsolute { .. }
        | traversal::Error::AbsolutePathWithoutShed { .. }
        | traversal::Error::DestinationOutsideRoot { .. } => PhaseOutcome::BlockingErrors,
        traversal::Error::OverlappingLeaves { .. } => PhaseOutcome::NonBlockingErrors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Index;
    use pretty_assertions::assert_eq;
    use std::{collections::HashSet, env::VarError};

    fn expansion(index: Index, field: traversal::PathField) -> traversal::Error {
        traversal::Error::Expansion {
            index,
            field,
            source: shellexpand::path::LookupError {
                var_name: "UNSET".into(),
                cause: VarError::NotPresent,
            },
        }
    }

    #[test]
    fn gives_no_errors_without_errors() {
        assert_eq!(outcome(&[]), PhaseOutcome::NoErrors);
    }

    #[test]
    fn blocks_the_run_for_an_item_expansion() {
        for field in [traversal::PathField::Path, traversal::PathField::Shed] {
            assert_eq!(
                outcome(&[expansion(([0], [0]).into(), field)]),
                PhaseOutcome::BlockingErrors
            );
        }
    }

    #[test]
    fn blocks_the_run_for_a_manifest_expansion() {
        for field in [traversal::PathField::Path, traversal::PathField::Shed] {
            assert_eq!(
                outcome(&[expansion(
                    Index {
                        manifest: vec![0],
                        item: Vec::new(),
                    },
                    field
                )]),
                PhaseOutcome::BlockingErrors
            );
        }
    }

    #[test]
    fn blocks_the_run_for_a_non_root_item_with_absolute_path_and_shed() {
        assert_eq!(
            outcome(&[traversal::Error::NonRootPathShedBothAbsolute {
                index: ([0], [0, 0]).into(),
            }]),
            PhaseOutcome::BlockingErrors
        );
    }

    #[test]
    fn blocks_the_run_for_an_absolute_path_without_shed() {
        assert_eq!(
            outcome(&[traversal::Error::AbsolutePathWithoutShed {
                index: ([0], [0]).into(),
            }]),
            PhaseOutcome::BlockingErrors
        );
    }

    #[test]
    fn blocks_the_run_for_a_destination_outside_the_root() {
        assert_eq!(
            outcome(&[traversal::Error::DestinationOutsideRoot {
                index: ([0], [0]).into(),
                root: "/root".into(),
                dst: "/elsewhere".into(),
            }]),
            PhaseOutcome::BlockingErrors
        );
    }

    #[test]
    fn does_not_block_the_run_for_overlapping_leaves() {
        assert_eq!(
            outcome(&[traversal::Error::OverlappingLeaves {
                indexes: HashSet::from([([0], [0]).into(), ([0], [1]).into()]),
            }]),
            PhaseOutcome::NonBlockingErrors
        );
    }

    #[test]
    fn blocks_the_run_for_overlapping_leaves_and_an_expansion() {
        assert_eq!(
            outcome(&[
                traversal::Error::OverlappingLeaves {
                    indexes: HashSet::from([([0], [0]).into(), ([0], [1]).into()]),
                },
                expansion(([0], [2]).into(), traversal::PathField::Path),
            ]),
            PhaseOutcome::BlockingErrors
        );
    }
}
