use super::model::*;
use crate::{manifest, scanning};

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
                status: ItemStatus::Initial,
                error: None,
                items: Vec::new(),
                entries: None,
            },
            manifest::ItemKind::Item(item) => Self {
                path: item.path,
                shed: item.shed,
                status: ItemStatus::Initial,
                error: None,
                items: item.items.into_iter().flatten().map(Self::from).collect(),
                entries: None,
            },
        }
    }
}

impl From<&scanning::Entry> for Entry {
    fn from(entry: &scanning::Entry) -> Self {
        let name = entry
            .dst()
            .file_name()
            .expect("scanned entry has a file name")
            .to_os_string();
        let entries = match entry {
            scanning::Entry::Directory { entries, .. } => {
                Some(entries.iter().map(Self::from).collect())
            }
            _ => None,
        };
        Self {
            name,
            status: EntryStatus::Initial,
            error: None,
            entries,
        }
    }
}
