use super::error::Error;
use crate::manifest::FullIndex as ManifestIndex;
use std::{collections::HashSet, ffi::OsString, path::PathBuf};

pub enum Direction {
    Put,
    Get,
}

#[derive(Debug, PartialEq)]
pub struct Plan {
    pub roots: Vec<Root>,
    pub errors: Vec<Error>,
}

#[derive(Debug, PartialEq)]
pub enum Root {
    Directory(RootDirectory),
    Leaf(RootLeaf),
}

#[derive(Debug, PartialEq)]
pub struct RootDirectory {
    pub manifest_indexes: HashSet<ManifestIndex>,
    pub dst: PathBuf,
    pub directories: Vec<NestedDirectory>,
    pub leaves: Vec<NestedLeaf>,
}

#[derive(Debug, PartialEq)]
pub struct RootLeaf {
    pub manifest_index: ManifestIndex,
    pub src: PathBuf,
    pub dst: PathBuf,
}

#[derive(Debug, PartialEq)]
pub struct NestedDirectory {
    pub manifest_indexes: HashSet<ManifestIndex>,
    pub dst: OsString,
    pub directories: Vec<NestedDirectory>,
    pub leaves: Vec<NestedLeaf>,
}

#[derive(Debug, PartialEq)]
pub struct NestedLeaf {
    pub manifest_index: ManifestIndex,
    pub src: PathBuf,
    pub dst: OsString,
}
