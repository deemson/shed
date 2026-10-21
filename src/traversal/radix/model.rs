use super::super::error::Error;
use crate::manifest::Index;
use iradix::unsync::Radix;
use std::{ffi::OsString, path::PathBuf};

#[derive(Default)]
pub(in crate::traversal) struct Plan {
    pub radix: Radix<OsString, Vec<Entry>>,
    pub errors: Vec<Error>,
}

#[derive(Debug, PartialEq, Clone)]
pub(in crate::traversal) struct Entry {
    pub dst_root: Option<PathBuf>,
    pub index: Index,
    pub src: PathBuf,
}
