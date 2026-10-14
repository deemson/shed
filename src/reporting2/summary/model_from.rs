use super::model::*;
use crate::manifest;

impl From<manifest::Manifest> for Manifest {
    fn from(manifest: manifest::Manifest) -> Self {
        Self {
            name: manifest.name,
            status: ManifestStatus::Initial,
            error: None,
            manifests: manifest.manifests.into_iter().map(Self::from).collect(),
            items: manifest.items.into_iter().map(Item::from).collect(),
        }
    }
}

impl From<manifest::ItemKind> for Item {
    fn from(item: manifest::ItemKind) -> Self {
        Self {
            items: match item {
                manifest::ItemKind::Path(_) => Vec::new(),
                manifest::ItemKind::Item(item) => {
                    item.items.into_iter().flatten().map(Self::from).collect()
                }
            },
        }
    }
}
