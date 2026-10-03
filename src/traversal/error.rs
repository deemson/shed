use std::collections::HashSet;

use crate::manifest::FullIndex;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("non-root item has both path and shed set to absolute paths")]
    NonRootPathShedBothAbsolute { index: FullIndex },

    #[error("overlapping leaves detected")]
    OverlappingLeaves { indexes: HashSet<FullIndex> },
}
