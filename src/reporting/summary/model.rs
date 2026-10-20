use crate::traversal;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Report {
    pub direction: Option<traversal::Direction>,
    pub manifests: Vec<Manifest>,
}

#[derive(Debug)]
pub struct Manifest {
    pub name: String,
    pub location: PathBuf,
    pub path: Option<String>,
    pub shed: Option<String>,
    pub status: ManifestStatus,
    pub error: Option<String>,
    pub manifests: Vec<Manifest>,
    pub items: Vec<Item>,
}

#[derive(Debug, PartialEq)]
pub enum ManifestStatus {
    Initial,
    Resolving,
    Resolved,
}

#[derive(Debug)]
pub struct Item {
    pub path: String,
    pub shed: Option<String>,
    pub error: Option<String>,
    pub items: Vec<Item>,
}
