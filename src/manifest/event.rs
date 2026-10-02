use super::error::Error;
use super::index::Index;

#[derive(Debug)]
pub enum Event {
    Started { index: Index },
    Resolved { index: Index },
    Error { index: Index, error: Error },
    Done,
}
