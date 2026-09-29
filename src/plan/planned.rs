use std::path::PathBuf;

pub(super) struct Operations {
    directory_removals: Option<Vec<DirectoryRemoval>>,
    file_copies: Option<Vec<FileCopy>>,
}

pub(super) struct DirectoryRemoval {
    dst: PathBuf,
}

pub(super) struct FileCopy {
    src: PathBuf,
    dst: PathBuf,
}
