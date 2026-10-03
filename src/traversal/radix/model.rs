use super::super::error::Error;
use super::super::model::Leaf;
use iradix::unsync::Radix;
use std::ffi::OsString;

pub(in crate::traversal) struct Plan {
    pub(super) radix: Radix<OsString, Leaf>,
    pub(super) errors: Vec<Error>,
}
