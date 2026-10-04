use super::error::Error;
use crate::manifest::FullIndex;
use std::{ffi::OsString, path::PathBuf};

#[derive(Clone)]
pub enum Direction {
    Put,
    Get,
}

impl Direction {
    pub fn resolve(&self, path: PathBuf, shed: PathBuf) -> (PathBuf, PathBuf) {
        let (src, dst) = match self {
            Direction::Put => (path, shed),
            Direction::Get => (shed, path),
        };
        (src, dst)
    }
}

#[derive(Debug, PartialEq)]
pub struct Plan {
    pub roots: Vec<Root>,
    pub errors: Vec<Error>,
}

#[derive(Debug, PartialEq)]
pub enum Root {
    Directory {
        dst: PathBuf,
        directories: Vec<Directory>,
        leaves: Vec<Leaf>,
    },
    Leaf {
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

#[derive(Debug, PartialEq, Clone)]
pub struct Leaf {
    pub index: FullIndex,
    pub src: PathBuf,
    pub dst: OsString,
}
