use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("")]
    SrcDirDstNot,
    #[error("")]
    DstDirSrcNot,
    #[error("failed to walk source directory: {source}")]
    WalkDirectory {
        #[source]
        source: async_walkdir::Error,
    },
    #[error("failed to inspect source path: {source}")]
    InspectSrcPath {
        #[source]
        source: io::Error,
    },
}
