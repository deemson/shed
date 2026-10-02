use std::collections::HashSet;

use crate::manifest::FullIndex as ManifestIndex;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("non-root item has both path and shed set to absolute paths")]
    NonRootPathShedBothAbsolute { manifest_index: ManifestIndex },

    #[error("overlapping leaves detected")]
    OverlappingLeaves {
        manifest_indexes: HashSet<ManifestIndex>,
    },
}
