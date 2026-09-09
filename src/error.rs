use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(String, #[source] std::io::Error),

    #[error("failed to parse {0}: {1}")]
    Yaml(PathBuf, #[source] yaml_serde::Error),

    #[error("invalid manifest {0}: {1}")]
    InvalidManifest(PathBuf, String),

    #[error("include cycle detected: {0}")]
    IncludeCycle(String),

    #[error("{0}")]
    InvalidConfig(String),

    #[error("failed to install Ctrl-C handler: {0}")]
    CtrlC(#[source] ctrlc::Error),
}
