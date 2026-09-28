use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::allocation::AllocationInfo;
use crate::evidence::Evidence;
use crate::risk::{RecommendationValue, RiskLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CandidateCategory {
    Cache,
    Log,
    Temporary,
    BuildArtifact,
    PackageManagerCache,
    SimulatorData,
    ContainerData,
    Download,
    Installer,
    ApplicationLeftover,
    Duplicate,
    UserDocument,
    Backup,
    VirtualMachine,
    CloudManaged,
    Unknown,
}

impl std::fmt::Display for CandidateCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CandidateCategory::Cache => write!(f, "CACHE"),
            CandidateCategory::Log => write!(f, "LOG"),
            CandidateCategory::Temporary => write!(f, "TEMPORARY"),
            CandidateCategory::BuildArtifact => write!(f, "BUILD_ARTIFACT"),
            CandidateCategory::PackageManagerCache => write!(f, "PACKAGE_MANAGER_CACHE"),
            CandidateCategory::SimulatorData => write!(f, "SIMULATOR_DATA"),
            CandidateCategory::ContainerData => write!(f, "CONTAINER_DATA"),
            CandidateCategory::Download => write!(f, "DOWNLOAD"),
            CandidateCategory::Installer => write!(f, "INSTALLER"),
            CandidateCategory::ApplicationLeftover => write!(f, "APPLICATION_LEFTOVER"),
            CandidateCategory::Duplicate => write!(f, "DUPLICATE"),
            CandidateCategory::UserDocument => write!(f, "USER_DOCUMENT"),
            CandidateCategory::Backup => write!(f, "BACKUP"),
            CandidateCategory::VirtualMachine => write!(f, "VIRTUAL_MACHINE"),
            CandidateCategory::CloudManaged => write!(f, "CLOUD_MANAGED"),
            CandidateCategory::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Deterministic candidate identifier (e.g. SHA-256 slice of canonical path).
    pub id: String,

    /// Absolute canonical path on filesystem.
    pub path: PathBuf,

    /// Semantic classification.
    pub category: CandidateCategory,

    /// Physical vs logical byte accounting.
    pub allocation: AllocationInfo,

    /// Evaluated risk level.
    pub risk: RiskLevel,

    /// Evaluated recommendation value.
    pub value: RecommendationValue,

    /// Calibrated confidence score in [0.0, 1.0].
    pub confidence_score: f32,

    /// Collection of corroborating evidence.
    pub evidence: Vec<Evidence>,

    /// Whether this item can be rebuilt/regenerated if reclaimed.
    pub reconstructable: bool,

    /// Human-readable explanation of what happens if removed.
    pub rebuild_consequence: Option<String>,

    /// Inode on disk (for TOCTOU check).
    pub inode: u64,

    /// Device ID on disk (for TOCTOU check).
    pub device_id: u64,

    /// Last modification timestamp in unix seconds.
    pub mtime_sec: i64,
}

impl Candidate {
    pub fn is_auto_cleanable(&self) -> bool {
        self.risk == RiskLevel::Safe && self.category != CandidateCategory::Unknown
    }
}
