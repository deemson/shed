use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read metadata at {}: {source}", path.display())]
    MetadataIo {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read directory at {}: {source}", path.display())]
    ReadDirIo {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("unsupported filesystem type at {}", path.display())]
    UnsupportedFileType { path: PathBuf },
}
