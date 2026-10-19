use super::model::*;
use crate::manifest;

impl From<manifest::Manifest> for Manifest {
    fn from(manifest: manifest::Manifest) -> Self {
        Self {
            name: manifest.name,
            location: manifest.location,
            path: manifest.path,
            shed: manifest.shed,
            status: ManifestStatus::Initial,
            error: None,
            manifests: manifest.manifests.into_iter().map(Self::from).collect(),
            items: manifest.items.into_iter().map(Item::from).collect(),
        }
    }
}

impl From<manifest::ItemKind> for Item {
    fn from(item: manifest::ItemKind) -> Self {
        match item {
            manifest::ItemKind::Path(path) => Self {
                path,
                shed: None,
                error: None,
                items: Vec::new(),
            },
            manifest::ItemKind::Item(item) => Self {
                path: item.path,
                shed: item.shed,
                error: None,
                items: item.items.into_iter().flatten().map(Self::from).collect(),
            },
        }
    }
}
