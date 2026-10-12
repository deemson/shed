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
            items: manifest.items.iter().map(Item::from).collect(),
        }
    }
}

impl From<&manifest::ItemKind> for Item {
    fn from(item: &manifest::ItemKind) -> Self {
        Self {
            error: None,
            items: match item {
                manifest::ItemKind::Path(_) => Vec::new(),
                manifest::ItemKind::Item(item) => {
                    item.items.iter().flatten().map(Self::from).collect()
                }
            },
        }
    }
}
