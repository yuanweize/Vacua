use crate::ecosystem::DeveloperEcosystem;
use crate::evidence::{ActiveProjectState, RebuildConfidence, RebuildEvidence};
use crate::id::{DeveloperArtifactId, DeveloperProjectId};
use crate::kind::DeveloperArtifactKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeveloperArtifact {
    pub artifact_id: DeveloperArtifactId,
    pub project_id: DeveloperProjectId,
    #[serde(skip)]
    pub raw_relative_path: Vec<u8>,
    pub display_name: String,
    pub display_path: String,
    pub ecosystem: DeveloperEcosystem,
    pub artifact_kind: DeveloperArtifactKind,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub confirmed_reclaim_lower_bound: u64,
    pub estimated_reclaim: u64,
    pub physical_sharing_uncertainty: bool,
    pub rebuild_evidence: RebuildEvidence,
    pub candidate_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeveloperProject {
    pub project_id: DeveloperProjectId,
    #[serde(skip)]
    pub raw_relative_path: Vec<u8>,
    pub display_name: String,
    pub display_path: String,
    pub primary_ecosystem: DeveloperEcosystem,
    pub all_ecosystems: Vec<DeveloperEcosystem>,
    pub manifest_paths: Vec<String>,
    pub lockfile_paths: Vec<String>,
    pub artifacts: Vec<DeveloperArtifact>,
    pub total_logical_bytes: u64,
    pub total_allocated_bytes: u64,
    pub rebuild_confidence: RebuildConfidence,
    pub active_state: ActiveProjectState,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeveloperArtifactCoverage {
    pub supported_ecosystems: Vec<DeveloperEcosystem>,
    pub unclassified_candidate_directories: usize,
    pub skipped_items: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArtifactAnalysisGeneration {
    pub generation_id: String,
    pub root_id: String,
    pub root_path: String,
    pub observed_at: i64,
    pub projects: Vec<DeveloperProject>,
    pub total_logical_bytes: u64,
    pub total_allocated_bytes: u64,
    pub coverage: DeveloperArtifactCoverage,
}
