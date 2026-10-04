use super::super::{error::Error, model::Leaf};
use iradix::unsync::Radix;
use std::ffi::OsString;

#[derive(Default)]
pub(in crate::traversal) struct Plan {
    pub radix: Radix<OsString, Vec<Leaf>>,
    pub errors: Vec<Error>,
}
