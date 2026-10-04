use super::super::model::Leaf;
use iradix::unsync::Radix;
use std::ffi::OsString;

pub(in crate::traversal) type Plan = Radix<OsString, Vec<Leaf>>;
