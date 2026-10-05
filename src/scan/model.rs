use crate::manifest::FullIndex;
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq)]
pub enum Root {
    Directory {
        dst: PathBuf,
        directories: Vec<Directory>,
        leaves: Vec<Leaf>,
    },
    File {
        index: FullIndex,
        src: PathBuf,
        dst: PathBuf,
    },
}

#[derive(Debug, PartialEq)]
pub struct Directory {
    pub dst: OsString,
    pub directories: Vec<Directory>,
    pub leaves: Vec<Leaf>,
}

#[derive(Debug, PartialEq)]
pub enum Leaf {
    Directory {
        index: FullIndex,
        dst: OsString,
        nested: Vec<Nested>,
    },
    File {
        index: FullIndex,
        src: PathBuf,
        dst: OsString,
    },
}

#[derive(Debug, PartialEq)]
pub enum Nested {
    Directory { path: OsString, nested: Vec<Nested> },
    File { path: OsString },
}
