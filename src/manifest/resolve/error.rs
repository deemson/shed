use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("manifest was not found")]
    NotFound,

    #[error("failed to access manifest: {source}")]
    Io {
        #[source]
        source: io::Error,
    },

    #[error("failed to parse manifest: {source}")]
    Yaml {
        #[source]
        source: yaml_serde::Error,
    },

    #[error("include cycle detected")]
    Cycle,
}
