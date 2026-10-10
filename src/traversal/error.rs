use std::{collections::HashSet, path::PathBuf};
use crate::manifest::FullIndex;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("failed to expand {field}: {source}")]
    Expansion {
        /// An empty item index identifies a manifest-level field.
        index: FullIndex,
        field: PathField,
        #[source]
        source: shellexpand::path::LookupError<std::env::VarError>,
    },

    #[error("non-root item has both path and shed set to absolute paths")]
    NonRootPathShedBothAbsolute { index: FullIndex },

    #[error("overlapping leaves detected")]
    OverlappingLeaves { indexes: HashSet<FullIndex> },

    #[error("destination {dst:?} is outside declared root {root:?}")]
    DestinationOutsideRoot {
        index: FullIndex,
        root: PathBuf,
        dst: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathField {
    Path,
    Shed,
}

impl std::fmt::Display for PathField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Path => "path",
            Self::Shed => "shed",
        })
    }
}
