use std::{ffi::OsString, path::PathBuf};

pub enum Direction {
    Put,
    Get,
}

#[derive(Debug, PartialEq)]
pub struct Root {
    pub manifest_indexes: Vec<Vec<usize>>,
    pub dst: PathBuf,
    pub is_clean_dst: bool,
    pub directories: Vec<Directory>,
    pub files: Vec<File>,
}

#[derive(Debug, PartialEq)]
pub struct Directory {
    pub manifest_indexes: Vec<Vec<usize>>,
    pub dst: OsString,
    pub is_clean_dst: bool,
    pub directories: Vec<Directory>,
    pub files: Vec<File>,
}

#[derive(Debug, PartialEq)]
pub struct File {
    pub manifest_index: Vec<usize>,
    pub src: PathBuf,
    pub dst: OsString,
}
