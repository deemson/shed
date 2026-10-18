use super::error::Error;
use crate::manifest::FullIndex;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Event {
    NodeScanningStarted {
        index: FullIndex,
    },
    NodeScanningFinished {
        index: FullIndex,
        error: Option<Error>,
    },
    EntryScanningStarted {
        index: FullIndex,
        rel: PathBuf,
    },
    EntryScanningFinished {
        index: FullIndex,
        rel: PathBuf,
        error: Option<Error>,
    },
    Done,
}
