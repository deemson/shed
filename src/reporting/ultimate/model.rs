pub struct Report {
    pub manifests: Vec<Manifest>,
}

pub struct Manifest {
    pub manifests: Vec<Manifest>,
    pub name: String,
}
