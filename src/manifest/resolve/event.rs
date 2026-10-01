use super::error::Error;
use crate::manifest::Index;

#[derive(Debug)]
pub enum Event {
    Started { index: Index },
    Resolved { index: Index },
    Error { index: Index, error: Error },
    Done,
}
