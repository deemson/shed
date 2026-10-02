pub use super::yaml::ItemKind;
pub use super::yaml::Item;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub location: PathBuf,
    pub path: Option<String>,
    pub shed: Option<String>,
    pub manifests: Vec<Manifest>,
    pub items: Vec<ItemKind>,
}

