use std::path::PathBuf;
use crate::manifest::RootItem;

#[derive(Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub path: PathBuf,
    pub manifests: Vec<Manifest>,
    pub items: Vec<RootItem>,
}
