use std::{ffi::OsString, path::PathBuf};

pub enum Direction {
    Put,
    Get,
}

#[derive(Debug, PartialEq)]
pub struct Root {
    pub dst: PathBuf,
    pub is_clean_dst: bool,
    pub directories: Option<Vec<Directory>>,
    pub files: Option<Vec<File>>,
}

#[derive(Debug, PartialEq)]
pub struct Directory {
    pub dst: OsString,
    pub is_clean_dst: bool,
    pub directories: Option<Vec<Directory>>,
    pub files: Option<Vec<File>>,
}

#[derive(Debug, PartialEq)]
pub struct File {
    pub src: PathBuf,
    pub dst: OsString,
}
