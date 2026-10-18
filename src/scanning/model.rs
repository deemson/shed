use crate::manifest::FullIndex;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum Node {
    EnsureDirectory {
        dst: PathBuf,
        nodes: Vec<Node>,
    },
    ReplaceFile {
        index: FullIndex,
        src: PathBuf,
        dst: PathBuf,
    },
    ReplaceDirectory {
        index: FullIndex,
        src: PathBuf,
        dst: PathBuf,
        entries: Vec<Entry>,
    },
}

#[derive(Debug, PartialEq)]
pub enum Entry {
    Directory {
        dst: PathBuf,
        entries: Vec<Entry>,
    },
    File {
        src: PathBuf,
        dst: PathBuf,
    },
}
