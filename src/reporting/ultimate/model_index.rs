use super::model::*;
use crate::manifest::IndexBit;

impl Report {
    pub fn get_manifest(&self, index: &[IndexBit]) -> &Manifest {
        let (&first, rest) = index.split_first().expect("manifest index is empty");
        self.manifests[first].get_manifest(rest)
    }
}

impl Manifest {
    pub fn get_manifest(&self, index: &[IndexBit]) -> &Self {
        index
            .iter()
            .fold(self, |manifest, &index| &manifest.manifests[index])
    }
}
