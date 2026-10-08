use crate::manifest::FullIndex;
use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to ensure directory at {}: {source}", path.display())]
    EnsureDirectoryIo {
        path: PathBuf,
        #[source]
        source: io::Error,
        child_indexes: Vec<FullIndex>,
    },
    #[error("failed to replace from {} to {}: {source}", src.display(), dst.display())]
    ReplaceIo {
        src: PathBuf,
        dst: PathBuf,
        #[source]
        source: io::Error,
    },
}
