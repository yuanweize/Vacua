use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// High-level macOS storage summary with deterministic APFS metrics.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StorageSummaryV1 {
    pub schema_version: String,
    pub observed_at: String,
    pub target_path: String,
    pub mount_point: String,
    pub filesystem_type: String,
    pub total_space_bytes: u64,
    pub free_space_bytes: u64,
    pub available_space_bytes: u64,
    pub purgeable_space_bytes: Option<u64>,
    pub pressure_level: String,
    pub candidate_count: usize,
    pub candidate_reclaim_bytes: u64,
    pub candidate_reviewable_bytes: u64,
    pub index_freshness: Option<String>,
    pub is_stale: bool,
}

/// Compact candidate summary for list views.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CandidateSummaryV1 {
    pub candidate_id: String,
    pub display_path: String,
    pub category: String,
    pub risk: String,
    pub allocated_bytes: u64,
    pub reclaim_estimate_bytes: u64,
    pub reconstructable: bool,
}

/// Paginated candidate list response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CandidateListResponseV1 {
    pub schema_version: String,
    pub items: Vec<CandidateSummaryV1>,
    pub total_count: usize,
    pub limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// In-depth candidate explanation and risk evidence.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CandidateDetailV1 {
    pub schema_version: String,
    pub candidate_id: String,
    pub display_path: String,
    pub category: String,
    pub risk: String,
    pub allocated_bytes: u64,
    pub logical_bytes: u64,
    pub reconstructable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebuild_consequence: Option<String>,
    pub cost_model_status: String,
    pub evidence_count: usize,
    pub evidence_signals: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_mtime: Option<i64>,
}

/// Snapshot summary item.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotSummaryV1 {
    pub snapshot_id: String,
    pub name: String,
    pub root_path: String,
    pub created_at: String,
    pub total_files: u64,
    pub allocated_bytes: u64,
}

/// Paginated snapshot list response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotListResponseV1 {
    pub schema_version: String,
    pub items: Vec<SnapshotSummaryV1>,
    pub total_count: usize,
    pub limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Subtree delta for diff views.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubtreeDeltaV1 {
    pub display_path: String,
    pub delta_bytes: i64,
    pub files_delta: i64,
}

/// Allocation difference between two snapshots or live filesystem.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotDiffV1 {
    pub schema_version: String,
    pub base_snapshot: String,
    pub target_snapshot: String,
    pub allocated_delta_bytes: i64,
    pub logical_delta_bytes: i64,
    pub files_delta: i64,
    pub top_growing: Vec<SubtreeDeltaV1>,
    pub top_shrinking: Vec<SubtreeDeltaV1>,
}

/// Discovered application summary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplicationSummaryV1 {
    pub application_id: String,
    pub app_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_path: Option<String>,
    pub installed: bool,
    pub orphan_confidence: String,
}

/// Paginated application list response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplicationListResponseV1 {
    pub schema_version: String,
    pub items: Vec<ApplicationSummaryV1>,
    pub total_count: usize,
    pub limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Application artifact item in detail view.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplicationArtifactV1 {
    pub artifact_id: String,
    pub display_path: String,
    pub kind: String,
    pub allocated_bytes: u64,
    pub risk: String,
}

/// Discovered application detail and residual artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplicationDetailV1 {
    pub schema_version: String,
    pub application_id: String,
    pub app_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_path: Option<String>,
    pub installed: bool,
    pub orphan_confidence: String,
    pub artifact_count: usize,
    pub estimated_reclaim_bytes: u64,
    pub artifacts: Vec<ApplicationArtifactV1>,
}

/// Member of an exact duplicate group.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DuplicateMemberV1 {
    pub member_id: String,
    pub display_path: String,
    pub physical_relation: String,
    pub allocated_bytes: u64,
    pub kernel_private_bytes: u64,
    pub is_cloud_placeholder: bool,
}

/// Duplicate group summary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DuplicateGroupSummaryV1 {
    pub group_id: String,
    pub member_count: usize,
    pub file_size: u64,
    pub logical_duplicate_bytes: u64,
    pub kernel_private_bytes: u64,
    pub confirmed_reclaim_lower_bound: u64,
    pub estimated_reclaim: u64,
    pub content_identity_verified: bool,
    pub algorithm: String,
}

/// Paginated duplicate groups list response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DuplicateListResponseV1 {
    pub schema_version: String,
    pub items: Vec<DuplicateGroupSummaryV1>,
    pub total_count: usize,
    pub limit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Detailed duplicate group with members and APFS physical sharing truth.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DuplicateGroupDetailV1 {
    pub schema_version: String,
    pub group_id: String,
    pub file_size: u64,
    pub logical_duplicate_bytes: u64,
    pub kernel_private_bytes: u64,
    pub confirmed_reclaim_lower_bound: u64,
    pub estimated_reclaim: u64,
    pub reclaim_upper_bound: u64,
    pub content_identity_verified: bool,
    pub algorithm: String,
    pub members: Vec<DuplicateMemberV1>,
}

/// Cleanup simulation consequences without filesystem mutation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CleanupSimulationV1 {
    pub schema_version: String,
    pub candidate_count: usize,
    pub immediate_reclaim_bytes: u64,
    pub eventual_reclaim_estimate_bytes: u64,
    pub highest_risk: String,
    pub rebuild_consequences: Vec<String>,
    pub blocked_items: Vec<String>,
    pub cost_model_status: String,
}

/// Item in a proposed immutable CleanupPlan.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlanProposalItemV1 {
    pub candidate_id: String,
    pub display_path: String,
    pub action: String,
    pub risk: String,
    pub estimated_bytes: u64,
}

/// Proposed immutable CleanupPlan v2 with cryptographic hash.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CleanupPlanProposalV1 {
    pub schema_version: String,
    pub plan_schema_version: u32,
    pub plan_id: String,
    pub plan_hash: String,
    pub created_at: String,
    pub item_count: usize,
    pub highest_risk: String,
    pub estimated_eventual_reclaim_bytes: u64,
    pub immediate_reclaim_bytes: u64,
    pub preservation_guard_count: usize,
    pub items: Vec<PlanProposalItemV1>,
    pub proposal_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialized_plan: Option<String>,
}

/// Server capabilities and capability isolation report.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServerCapabilitiesV1 {
    pub schema_version: String,
    pub server_version: String,
    pub mcp_protocol_generation: String,
    pub platform: String,
    pub transport: String,
    pub path_disclosure_mode: String,
    pub allowed_roots: Vec<String>,
    pub mutation_authority: bool,
    pub executor_linked: bool,
    pub read_only_tier: bool,
    pub analyze_only_tier: bool,
    pub propose_only_tier: bool,
    pub index_available: bool,
    pub snapshot_engine_available: bool,
    pub duplicate_engine_available: bool,
    pub app_evidence_graph_available: bool,
    pub apple_foundation_models_status: String,
}

/// Recent transaction entry in history summary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HistoryTransactionV1 {
    pub transaction_id: String,
    pub plan_hash: String,
    pub timestamp: i64,
    pub total_items: usize,
    pub successful_count: usize,
    pub reclaimed_bytes: u64,
}

/// History journal summary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HistorySummaryV1 {
    pub schema_version: String,
    pub total_transactions: usize,
    pub total_reclaimed_bytes: u64,
    pub recent_transactions: Vec<HistoryTransactionV1>,
}

/// Cryptographic verification report for audit history.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HistoryVerificationV1 {
    pub schema_version: String,
    pub total_records: usize,
    pub is_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broken_record_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
}
