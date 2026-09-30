use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArtifactError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Artifact generation not found: {0}")]
    GenerationNotFound(String),

    #[error("Developer project not found: {0}")]
    ProjectNotFound(String),

    #[error("Developer artifact not found: {0}")]
    ArtifactNotFound(String),

    #[error("Dataless cloud placeholder encountered: {0}")]
    DatalessPlaceholder(String),

    #[error("Operation cancelled or timed out")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, ArtifactError>;
