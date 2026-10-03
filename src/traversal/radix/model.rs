use super::super::{error::Error, model::Leaf};
use iradix::unsync::Radix;
use std::ffi::OsString;

pub(in crate::traversal) struct Plan {
    pub radix: Radix<OsString, Leaf>,
    pub errors: Vec<Error>,
}
