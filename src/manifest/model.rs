pub use super::yaml::Item;
pub use super::yaml::ItemKind;
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
