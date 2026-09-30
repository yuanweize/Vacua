use thiserror::Error;

#[derive(Error, Debug)]
pub enum TreeError {
    #[error("Storage counter overflow during tree aggregation")]
    CounterOverflow,

    #[error("Tree generation not found or no longer available: {0}")]
    GenerationNotFound(String),

    #[error("Storage node not found in generation: {0}")]
    NodeNotFound(String),

    #[error("Snapshot root mismatch: tree root '{tree_root}' does not match snapshot root '{snapshot_root}'")]
    SnapshotRootMismatch {
        tree_root: String,
        snapshot_root: String,
    },

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, TreeError>;
