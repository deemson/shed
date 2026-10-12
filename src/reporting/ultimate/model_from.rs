use super::model::*;
use crate::manifest;

impl From<&[manifest::Manifest]> for Report {
    fn from(manifests: &[manifest::Manifest]) -> Self {
        Self {
            manifests: manifests.iter().map(Manifest::from).collect(),
        }
    }
}

impl From<&manifest::Manifest> for Manifest {
    fn from(manifest: &manifest::Manifest) -> Self {
        Self {
            manifests: manifest.manifests.iter().map(Self::from).collect(),
            name: manifest.name.clone(),
        }
    }
}
