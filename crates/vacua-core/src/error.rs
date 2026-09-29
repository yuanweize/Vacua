use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReclaimError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Safety invariant violation: {0}")]
    SafetyViolation(String),

    #[error("TOCTOU validation mismatch for target {path}: {reason}")]
    ToctouMismatch { path: String, reason: String },

    #[error("Plan integrity check failed: expected hash {expected}, got {computed}")]
    PlanIntegrityFailure { expected: String, computed: String },

    #[error("Plan execution refused: {reason}")]
    PlanExecutionRefused { reason: String },

    #[error("Preservation guard failed for kept copy {path}: {reason}")]
    PreservationGuardFailure { path: String, reason: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Rule parsing error: {0}")]
    RuleParse(String),

    #[error("General error: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, ReclaimError>;
