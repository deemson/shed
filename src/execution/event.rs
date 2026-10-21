use super::error::Error;
use crate::manifest::Index;
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
        index: Index,
        src: PathBuf,
        dst: PathBuf,
    },
    ReplaceFinished {
        index: Index,
        src: PathBuf,
        dst: PathBuf,
        error: Option<Error>,
    },
    Done,
}
