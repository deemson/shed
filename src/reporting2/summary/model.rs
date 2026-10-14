#[derive(Debug)]
pub struct Report {
    pub manifests: Vec<Manifest>,
}

#[derive(Debug)]
pub struct Manifest {
    pub name: String,
    pub status: ManifestStatus,
    pub error: Option<String>,
    pub manifests: Vec<Manifest>,
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub enum ManifestStatus {
    Initial,
    ResolutionStarted,
    ResolutionFinished,
}

#[derive(Debug)]
pub struct Item {
    pub items: Vec<Item>,
}
