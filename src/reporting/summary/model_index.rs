use super::model::*;
use crate::{index, manifest, scanning};

impl Report {
    pub(super) fn get_manifest_mut(&mut self, index: index::Ref<'_>) -> &mut Manifest {
        let (&first, rest) = index.split_first().expect("manifest index is empty");
        self.manifests[first].get_manifest_mut(rest)
    }
    pub(super) fn get_item_mut(&mut self, index: manifest::IndexRef<'_>) -> &mut Item {
        self.get_manifest_mut(index.manifest)
            .get_item_mut(index.item)
    }
    pub(super) fn get_entry_mut(&mut self, index: scanning::IndexRef<'_>) -> &mut Entry {
        self.get_manifest_mut(index.manifest)
            .get_item_mut(index.item)
            .get_entry_mut(index.entry)
    }
}

impl Manifest {
    fn get_manifest_mut(&mut self, index: index::Ref<'_>) -> &mut Self {
        index
            .iter()
            .fold(self, |manifest, &bit| &mut manifest.manifests[bit])
    }
    fn get_item_mut(&mut self, index: index::Ref<'_>) -> &mut Item {
        let (&first, rest) = index.split_first().expect("item index is empty");
        self.items[first].get_item_mut(rest)
    }
}

impl Item {
    fn get_item_mut(&mut self, index: index::Ref<'_>) -> &mut Self {
        index.iter().fold(self, |item, &bit| &mut item.items[bit])
    }
    fn get_entry_mut(&mut self, index: index::Ref<'_>) -> &mut Entry {
        let (&first, rest) = index.split_first().expect("entry index is empty");
        let Some(ref mut entries) = self.entries else {
            panic!("item has no entries");
        };
        entries[first].get_entry_mut(rest)
    }
}

impl Entry {
    fn get_entry_mut(&mut self, index: index::Ref<'_>) -> &mut Self {
        index.iter().fold(self, |entry, &bit| {
            let Some(ref mut entries) = entry.entries else {
                panic!("entry index points into a file")
            };
            &mut entries[bit]
        })
    }
}
