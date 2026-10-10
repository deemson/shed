use crate::manifest::FullIndex;
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to ensure directory: {source}")]
    EnsureDirectoryIo {
        #[source]
        source: io::Error,
        child_indexes: Vec<FullIndex>,
    },
    #[error("failed to replace: {source}")]
    ReplaceIo {
        #[source]
        source: io::Error,
    },
}
