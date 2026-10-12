use crate::traversal;

pub struct Report {
    pub manifests: Vec<Manifest>,
}

pub struct Manifest {
    pub manifests: Vec<Manifest>,
    pub items: Vec<Item>,
}

pub struct Item {
    pub error: Option<traversal::Error>,
    pub items: Vec<Item>,
}
