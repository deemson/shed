use super::error::Error;
use crate::index::Index;

#[derive(Debug)]
pub enum Event {
    ResolutionStarted { index: Index },
    ResolutionFinished { index: Index, error: Option<Error> },
    Done,
}
