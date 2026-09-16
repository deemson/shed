use std::path::PathBuf;

use super::serde::RootItem;

pub struct Manifest {
    pub name: String,
    pub path: PathBuf,
    pub manifests: Vec<Manifest>,
    pub items: Vec<RootItem>,
}
