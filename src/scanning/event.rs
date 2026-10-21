use super::error::Error;
use crate::manifest::Index;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Event {
    NodeScanningStarted {
        index: Index,
    },
    NodeScanningFinished {
        index: Index,
        error: Option<Error>,
    },
    EntryScanningStarted {
        index: Index,
        rel: PathBuf,
    },
    EntryScanningFinished {
        index: Index,
        rel: PathBuf,
        error: Option<Error>,
    },
    Done,
}
