use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(String, #[source] std::io::Error),

    #[error("failed to parse {0}: {1}")]
    Yaml(PathBuf, #[source] yaml_serde::Error),

    #[error("inheritance cycle detected: {0}")]
    InheritanceCycle(String),

    #[error("cannot run abstract collection directly: {0:?}")]
    AbstractCollection(PathBuf),

    #[error("item not found: {0:?}")]
    ItemNotFound(PathBuf),

    #[error("{0}")]
    MissingArg(String),
}
