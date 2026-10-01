pub mod allocation;
pub mod candidate;
pub mod cost;
pub mod error;
pub mod evidence;
pub mod evidence_graph;
pub mod fs;
pub mod invariants;
pub mod pressure;
pub mod rescue;
pub mod risk;

pub use allocation::AllocationInfo;
pub use candidate::{Candidate, CandidateCategory};
pub use cost::{CleanupSimulation, CostTier, RebuildCost, ReclaimCost};
pub use error::{ReclaimError, Result};
pub use evidence::{Evidence, EvidenceSource};
pub use evidence_graph::{
    ApplicationEvidenceGraph, EdgeKind, GraphEdge, GraphNode, NodeKind, OrphanConfidence,
    OrphanEvaluation,
};
pub use fs::{
    classify_file_type, open_regular_file_safely, query_file_identity, FileIdentity, FileKind,
};
pub use invariants::{enforce_safety_invariants, is_protected_path};
pub use pressure::{
    evaluate_storage_pressure, query_volume_status, StoragePressure, VolumeStorageStatus,
};
pub use rescue::{
    build_storage_rescue_plan, compute_volume_accounting, group_candidates, CandidateGroup,
    ProtectedSummary, StorageDomain, StorageRescuePlan, WholeVolumeAccounting,
};
pub use risk::{RecommendationValue, RiskLevel};
