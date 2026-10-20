use super::model::*;
use crate::manifest;

impl Report {
    pub fn new(manifests: impl IntoIterator<Item = manifest::Manifest>) -> Self {
        Self {
            direction: None,
            manifests: manifests.into_iter().map(Manifest::from).collect(),
        }
    }
}
