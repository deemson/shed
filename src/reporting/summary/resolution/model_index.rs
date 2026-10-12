use super::model::*;
use crate::manifest;

impl Report {
    pub(super) fn get_mut(&mut self, index: manifest::IndexRef<'_>) -> &mut Manifest {
        let (&first, rest) = index.split_first().expect("manifest index is empty");
        self.manifests[first].get_mut(rest)
    }
}

impl Manifest {
    pub(super) fn get_mut(&mut self, index: manifest::IndexRef<'_>) -> &mut Self {
        index
            .iter()
            .fold(self, |manifest, &index| &mut manifest.manifests[index])
    }
}
