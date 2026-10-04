use super::super::{error::Error, model::Leaf};
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
    pub leaf: Leaf,
}
