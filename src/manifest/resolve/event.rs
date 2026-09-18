use super::error::Error;

#[derive(Debug)]
pub enum Event {
    Started { manifest: Vec<usize> },
    Resolved { manifest: Vec<usize> },
    Error { manifest: Vec<usize>, error: Error },
    Done,
}
