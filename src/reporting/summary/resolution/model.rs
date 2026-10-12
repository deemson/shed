use crate::manifest;

pub struct Report {
    pub is_done: bool,
    pub has_errors: bool,
    pub manifests: Vec<Manifest>,
}

pub struct Manifest {
    pub state: ManifestState,
    pub error: Option<manifest::Error>,
    pub manifests: Vec<Manifest>,
}

pub enum ManifestState {
    Initial,
    Started,
    Finished,
}
