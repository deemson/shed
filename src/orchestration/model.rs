#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PhaseOutcome {
    NoErrors,
    NonBlockingErrors,
    BlockingErrors,
}
