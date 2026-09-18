use super::error::Error;

#[derive(Debug)]
pub enum Event {
    Started { index: Vec<usize> },
    Resolved { index: Vec<usize> },
    Error { index: Vec<usize>, error: Error },
    Done,
}
