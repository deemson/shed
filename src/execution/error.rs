use crate::manifest::Index;
use std::{collections::HashSet, io};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to ensure directory: {source}")]
    EnsureDirectoryIo {
        #[source]
        source: io::Error,
        affected: HashSet<Index>,
    },
    #[error("failed to replace: {source}")]
    ReplaceIo {
        #[source]
        source: io::Error,
    },
}
