use super::{error::Error, index::Index};
use crate::manifest;

#[derive(Debug)]
pub enum Event {
    NodeScanningStarted {
        index: manifest::Index,
    },
    NodeScanningFinished {
        index: manifest::Index,
        error: Option<Error>,
    },
    EntryScanningStarted {
        index: Index,
    },
    EntryScanningFinished {
        index: Index,
        error: Option<Error>,
    },
    Done,
}
