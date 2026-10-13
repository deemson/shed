#[derive(Debug)]
pub struct Report {
    pub manifests: Vec<Manifest>,
}

#[derive(Debug)]
pub struct Manifest {
    pub name: String,
    pub manifests: Vec<Manifest>,
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub struct Item {
    pub items: Vec<Item>,
}
