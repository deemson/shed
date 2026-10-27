use crate::manifest::Index;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum Node {
    EnsureDirectory {
        dst: PathBuf,
        nodes: Vec<Node>,
    },
    ReplaceFile {
        index: Index,
        src: PathBuf,
        dst: PathBuf,
    },
    ReplaceDirectory {
        index: Index,
        src: PathBuf,
        dst: PathBuf,
        entries: Vec<Entry>,
    },
}

#[derive(Debug, PartialEq)]
pub enum Entry {
    Directory { dst: PathBuf, entries: Vec<Entry> },
    File { src: PathBuf, dst: PathBuf },
    Unknown { src: PathBuf, dst: PathBuf },
}

impl Entry {
    pub fn dst(&self) -> &Path {
        match self {
            Self::Directory { dst, .. } | Self::File { dst, .. } | Self::Unknown { dst, .. } => dst,
        }
    }

    pub fn contains_unknown(&self) -> bool {
        match self {
            Self::Directory { entries, .. } => entries.iter().any(Self::contains_unknown),
            Self::File { .. } => false,
            Self::Unknown { .. } => true,
        }
    }
}
