use super::error::Error;
use crate::{manifest::Index as ManifestIndex, scanning::Index as ScanIndex};
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
        kind: Kind,
        index: ManifestIndex,
        src: PathBuf,
        dst: PathBuf,
    },
    ReplaceFinished {
        index: ManifestIndex,
        src: PathBuf,
        dst: PathBuf,
        error: Option<Error>,
    },
    EntryStarted {
        kind: Kind,
        index: ScanIndex,
        src: PathBuf,
        dst: PathBuf,
    },
    EntryFinished {
        index: ScanIndex,
        src: PathBuf,
        dst: PathBuf,
        error: Option<Error>,
    },
    Done,
}

#[derive(Debug)]
pub enum Kind {
    Directory,
    File,
}
