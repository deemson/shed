use super::error::Error;
use crate::manifest::FullIndex;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Event {
    EnsureStarted {
        dst: PathBuf,
    },
    EnsureFinished {
        dst: PathBuf,
        error: Option<Error>,
    },
    ReplaceStarted {
        index: FullIndex,
        src: PathBuf,
        dst: PathBuf,
    },
    ReplaceFinished {
        index: FullIndex,
        src: PathBuf,
        dst: PathBuf,
        error: Option<Error>,
    },
    Done,
}
