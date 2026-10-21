use std::{collections::HashSet, path::PathBuf};
use crate::manifest::Index;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("failed to expand {field}: {source}")]
    Expansion {
        /// An empty item index identifies a manifest-level field.
        index: Index,
        field: PathField,
        #[source]
        source: shellexpand::path::LookupError<std::env::VarError>,
    },

    #[error("non-root item has both path and shed set to absolute paths")]
    NonRootPathShedBothAbsolute { index: Index },

    #[error("overlapping leaves detected")]
    OverlappingLeaves { indexes: HashSet<Index> },

    #[error("destination {dst:?} is outside declared root {root:?}")]
    DestinationOutsideRoot {
        index: Index,
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
