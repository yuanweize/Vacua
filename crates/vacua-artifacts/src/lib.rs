pub mod ecosystem;
pub mod error;
pub mod evidence;
pub mod id;
pub mod kind;
pub mod model;
pub mod persistence;
pub mod scanner;

pub use ecosystem::DeveloperEcosystem;
pub use error::{ArtifactError, Result};
pub use evidence::{ActiveProjectState, RebuildConfidence, RebuildEvidence};
pub use id::{path_to_raw_bytes, DeveloperArtifactId, DeveloperProjectId};
pub use kind::DeveloperArtifactKind;
pub use model::{
    ArtifactAnalysisGeneration, DeveloperArtifact, DeveloperArtifactCoverage, DeveloperProject,
};
pub use persistence::ArtifactPersistence;
pub use scanner::DeveloperArtifactScanner;

#[cfg(test)]
mod tests;
