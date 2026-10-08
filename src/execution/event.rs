use super::error::Error;
use crate::manifest::FullIndex;

#[derive(Debug)]
pub enum Event {
    EnsureError { error: Error },
    ReplaceError { index: FullIndex, error: Error },
    Done,
}
