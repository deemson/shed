use super::model::*;
use crate::manifest;

impl Report {
    pub(super) fn get_manifest_mut(&mut self, index: manifest::IndexRef<'_>) -> &mut Manifest {
        let (&first, rest) = index.split_first().expect("manifest index is empty");
        self.manifests[first].get_manifest_mut(rest)
    }
    pub(super) fn get_item_mut(&mut self, index: manifest::FullIndexRef<'_>) -> &mut Item {
        let (&first, rest) = index
            .manifest
            .split_first()
            .expect("manifest index is empty");
        self.manifests[first].get_item_mut(manifest::FullIndexRef {
            manifest: rest,
            item: index.item,
        })
    }
}

impl Manifest {
    pub(super) fn get_manifest_mut(&mut self, index: manifest::IndexRef<'_>) -> &mut Self {
        index
            .iter()
            .fold(self, |manifest, &index| &mut manifest.manifests[index])
    }
    pub(super) fn get_item_mut(&mut self, index: manifest::FullIndexRef<'_>) -> &mut Item {
        let (&first, rest) = index.item.split_first().expect("item index is empty");
        self.get_manifest_mut(index.manifest).items[first].get_item_mut(rest)
    }
}

impl Item {
    pub(super) fn get_item_mut(&mut self, index: manifest::IndexRef<'_>) -> &mut Self {
        index
            .iter()
            .fold(self, |item, &index| &mut item.items[index])
    }
}

