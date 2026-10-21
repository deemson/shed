use super::model::*;
use crate::{index, manifest};

impl Report {
    pub(super) fn get_manifest_mut(&mut self, index: index::Ref<'_>) -> &mut Manifest {
        let (&first, rest) = index.split_first().expect("manifest index is empty");
        self.manifests[first].get_manifest_mut(rest)
    }
    pub(super) fn get_item_mut(&mut self, index: manifest::IndexRef<'_>) -> &mut Item {
        self.get_manifest_mut(index.manifest).get_item_mut(index.item)
    }
}

impl Manifest {
    pub(super) fn get_manifest_mut(&mut self, index: index::Ref<'_>) -> &mut Self {
        index
            .iter()
            .fold(self, |manifest, &index| &mut manifest.manifests[index])
    }
    pub(super) fn get_item_mut(&mut self, index: index::Ref<'_>) -> &mut Item {
        let (&first, rest) = index.split_first().expect("item index is empty");
        self.items[first].get_item_mut(rest)
    }
}

impl Item {
    pub(super) fn get_item_mut(&mut self, index: index::Ref<'_>) -> &mut Self {
        index
            .iter()
            .fold(self, |item, &index| &mut item.items[index])
    }
}
