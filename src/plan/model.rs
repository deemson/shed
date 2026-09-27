use std::{ffi::OsString, path::PathBuf};

pub enum Direction {
    Put,
    Get,
}

#[derive(Debug, PartialEq)]
pub struct Directory {
    pub dst: PathBuf,
    pub is_to_be_cleaned: bool,
    pub directories: Option<Vec<Directory>>,
    pub files: Option<Vec<File>>,
}

#[derive(Debug, PartialEq)]
pub struct File {
    pub src: PathBuf,
    pub dst: OsString,
}
