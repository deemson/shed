use super::error::Error;
use crate::manifest::FullIndex;
use std::path::PathBuf;

#[derive(Clone, Debug)]
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
    pub roots: Vec<Node>,
    pub errors: Vec<Error>,
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Directory {
        dst: PathBuf,
        nodes: Vec<Node>,
    },
    Leaf {
        index: FullIndex,
        src: PathBuf,
        dst: PathBuf,
    },
}
