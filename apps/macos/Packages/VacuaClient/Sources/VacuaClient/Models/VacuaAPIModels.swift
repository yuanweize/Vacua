import Foundation

// MARK: - Schema Constants

public enum VacuaSchemas {
    public static let storageSummaryV1 = "vacua.mcp.storage-summary.v1"
    public static let candidateListV1 = "vacua.mcp.candidate-list.v1"
    public static let candidateDetailV1 = "vacua.mcp.candidate-detail.v1"
    public static let snapshotListV1 = "vacua.mcp.snapshot-list.v1"
    public static let snapshotDetailV1 = "vacua.mcp.snapshot-detail.v1"
    public static let snapshotDiffV1 = "vacua.mcp.snapshot-diff.v1"
    public static let applicationListV1 = "vacua.mcp.application-list.v1"
    public static let applicationDetailV1 = "vacua.mcp.application-detail.v1"
    public static let duplicateListV1 = "vacua.mcp.duplicate-list.v1"
    public static let duplicateGroupV1 = "vacua.mcp.duplicate-group.v1"
    public static let cleanupSimulationV1 = "vacua.mcp.cleanup-simulation.v1"
    public static let planProposalV1 = "vacua.mcp.plan-proposal.v1"
    public static let serverCapabilitiesV1 = "vacua.mcp.server-capabilities.v1"
    public static let historySummaryV1 = "vacua.mcp.history-summary.v1"
    public static let historyVerificationV1 = "vacua.mcp.history-verification.v1"
    public static let storageTreeAnalysisV1 = "vacua.mcp.storage-tree-analysis.v1"
    public static let storageTreePageV1 = "vacua.mcp.storage-tree-page.v1"
    public static let storageTreeNodeDetailV1 = "vacua.mcp.storage-tree-node-detail.v1"
}

// MARK: - Filter Enums

public enum McpRiskFilter: String, Codable, Sendable, CaseIterable {
    case safe
    case review
    case caution
    case protected
    case unknown

    public var displayName: String {
        switch self {
        case .safe: return "Safe"
        case .review: return "Review"
        case .caution: return "Caution"
        case .protected: return "Protected"
        case .unknown: return "Unknown"
        }
    }
}

public enum ApplicationFilter: String, Codable, Sendable, CaseIterable {
    case all
    case orphansOnly = "orphans-only"
    case installedOnly = "installed-only"

    public var displayName: String {
        switch self {
        case .all: return "All Applications"
        case .orphansOnly: return "Potential Residuals"
        case .installedOnly: return "Installed Applications"
        }
    }
}

// MARK: - Storage Summary

public struct StorageSummaryV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let observed_at: String
    public let target_path: String
    public let mount_point: String
    public let filesystem_type: String
    public let total_space_bytes: UInt64
    public let free_space_bytes: UInt64
    public let available_space_bytes: UInt64
    public let purgeable_space_bytes: UInt64?
    public let pressure_level: String
    public let candidate_count: Int
    public let candidate_reclaim_bytes: UInt64
    public let candidate_reviewable_bytes: UInt64
    public let candidate_confirmed_reclaim_bytes: UInt64
    public let candidate_estimated_reclaim_bytes: UInt64
    public let candidate_reclaim_upper_bound: UInt64
    public let index_freshness: String?
    public let is_stale: Bool

    public var usedSpaceBytes: UInt64 {
        if total_space_bytes > free_space_bytes {
            return total_space_bytes - free_space_bytes
        }
        return 0
    }
}

// MARK: - Candidates

public struct CandidateSummaryV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { candidate_id }

    public let candidate_id: String
    public let display_path: String
    public let category: String
    public let risk: String
    public let allocated_bytes: UInt64
    public let reclaim_estimate_bytes: UInt64
    public let reconstructable: Bool
    public let confirmed_reclaim_lower_bound: UInt64
    public let reclaim_upper_bound: UInt64
}

public struct CandidateListResponseV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let items: [CandidateSummaryV1]
    public let total_count: Int
    public let limit: Int
    public let next_cursor: String?
}

public struct CandidateDetailV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { candidate_id }

    public let schema_version: String
    public let candidate_id: String
    public let display_path: String
    public let category: String
    public let risk: String
    public let allocated_bytes: UInt64
    public let logical_bytes: UInt64
    public let confirmed_reclaim_lower_bound: UInt64
    public let estimated_reclaim_bytes: UInt64
    public let reclaim_upper_bound: UInt64
    public let reconstructable: Bool
    public let rebuild_consequence: String?
    public let cost_model_status: String
    public let evidence_count: Int
    public let evidence_signals: [String]
    public let observed_mtime: Int64?
}

// MARK: - Snapshots

public struct SnapshotSummaryV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { snapshot_id }

    public let snapshot_id: String
    public let name: String
    public let root_path: String
    public let created_at: String
    public let total_files: UInt64
    public let allocated_bytes: UInt64
}

public struct SnapshotDetailV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { snapshot_id }

    public let schema_version: String
    public let snapshot_id: String
    public let name: String
    public let root_path: String
    public let created_at: String
    public let total_files: UInt64
    public let total_dirs: UInt64
    public let logical_bytes: UInt64
    public let allocated_bytes: UInt64
    public let subtree_count: Int
}

public struct SnapshotListResponseV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let items: [SnapshotSummaryV1]
    public let total_count: Int
    public let limit: Int
    public let next_cursor: String?
}

public struct SubtreeDeltaV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { display_path }

    public let display_path: String
    public let delta_bytes: Int64
    public let files_delta: Int64
}

public struct SnapshotDiffV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let base_snapshot: String
    public let target_snapshot: String
    public let allocated_delta_bytes: Int64
    public let logical_delta_bytes: Int64
    public let files_delta: Int64
    public let top_growing: [SubtreeDeltaV1]
    public let top_shrinking: [SubtreeDeltaV1]
}

// MARK: - Applications

public struct ApplicationSummaryV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { application_id }

    public let application_id: String
    public let app_name: String
    public let display_path: String?
    public let installed: Bool
    public let orphan_confidence: String
}

public struct ApplicationListResponseV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let items: [ApplicationSummaryV1]
    public let total_count: Int
    public let limit: Int
    public let next_cursor: String?
}

public struct ApplicationArtifactV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { artifact_id }

    public let artifact_id: String
    public let display_path: String
    public let kind: String
    public let allocated_bytes: UInt64
    public let risk: String
}

public struct ApplicationDetailV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { application_id }

    public let schema_version: String
    public let application_id: String
    public let app_name: String
    public let display_path: String?
    public let installed: Bool
    public let orphan_confidence: String
    public let artifact_count: Int
    public let estimated_reclaim_bytes: UInt64
    public let artifacts: [ApplicationArtifactV1]
}

// MARK: - Duplicates

public struct DuplicateMemberV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { member_id }

    public let member_id: String
    public let display_path: String
    public let physical_relation: String
    public let allocated_bytes: UInt64
    public let kernel_private_bytes: UInt64
    public let kernel_private_bytes_known: Bool
    public let is_cloud_placeholder: Bool
}

public struct DuplicateGroupSummaryV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { group_id }

    public let group_id: String
    public let member_count: Int
    public let file_size: UInt64
    public let logical_duplicate_bytes: UInt64
    public let kernel_private_bytes: UInt64
    public let confirmed_reclaim_lower_bound: UInt64
    public let estimated_reclaim: UInt64
    public let content_identity_verified: Bool
    public let algorithm: String
    public let kernel_private_bytes_known_members: Int
    public let kernel_private_bytes_unknown_members: Int
}

public struct DuplicateListResponseV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let items: [DuplicateGroupSummaryV1]
    public let total_count: Int
    public let limit: Int
    public let next_cursor: String?
}

public struct DuplicateGroupDetailV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { group_id }

    public let schema_version: String
    public let group_id: String
    public let file_size: UInt64
    public let logical_duplicate_bytes: UInt64
    public let kernel_private_bytes: UInt64
    public let confirmed_reclaim_lower_bound: UInt64
    public let estimated_reclaim: UInt64
    public let reclaim_upper_bound: UInt64
    public let content_identity_verified: Bool
    public let algorithm: String
    public let kernel_private_bytes_known_members: Int
    public let kernel_private_bytes_unknown_members: Int
    public let members: [DuplicateMemberV1]
}

// MARK: - Simulation & Proposal

public struct CleanupSimulationV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { "\(schema_version)-\(candidate_count)-\(confirmed_lower_bound_bytes)" }

    public let schema_version: String
    public let candidate_count: Int
    public let immediate_reclaim_bytes: UInt64
    public let eventual_reclaim_estimate_bytes: UInt64
    public let confirmed_lower_bound_bytes: UInt64
    public let reclaim_upper_bound_bytes: UInt64
    public let highest_risk: String
    public let rebuild_consequences: [String]
    public let blocked_items: [String]
    public let cost_model_status: String
}

public struct PlanProposalItemV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { candidate_id }

    public let candidate_id: String
    public let display_path: String
    public let action: String
    public let risk: String
    public let estimated_bytes: UInt64
}

public struct CleanupPlanProposalV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { plan_id }

    public let schema_version: String
    public let plan_schema_version: UInt32
    public let plan_id: String
    public let plan_hash: String
    public let created_at: String
    public let item_count: Int
    public let highest_risk: String
    public let estimated_eventual_reclaim_bytes: UInt64
    public let immediate_reclaim_bytes: UInt64
    public let preservation_guard_count: Int
    public let items: [PlanProposalItemV1]
    public let proposal_status: String
    public let serialized_plan: String?
}

// MARK: - Server Capabilities

public struct ServerCapabilitiesV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let server_version: String
    public let mcp_protocol_generation: String
    public let platform: String
    public let transport: String
    public let path_disclosure_mode: String
    public let allowed_roots: [String]
    public let mutation_authority: Bool
    public let executor_linked: Bool
    public let read_only_tier: Bool
    public let analyze_only_tier: Bool
    public let propose_only_tier: Bool
    public let index_available: Bool
    public let snapshot_engine_available: Bool
    public let duplicate_engine_available: Bool
    public let app_evidence_graph_available: Bool
    public let apple_foundation_models_status: String
    public let plan_export_enabled: Bool
    public let system_app_metadata_enabled: Bool
    public let foundation_models_integration: String
    public let foundation_models_runtime_status: String
}

// MARK: - History

public struct HistoryTransactionV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { transaction_id }

    public let transaction_id: String
    public let plan_hash: String
    public let timestamp: Int64
    public let total_items: Int
    public let successful_count: Int
    public let reclaimed_bytes: UInt64
    public let bytes_moved_to_trash: UInt64
}

public struct HistorySummaryV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let total_transactions: Int
    public let total_reclaimed_bytes: UInt64
    public let total_bytes_moved_to_trash: UInt64
    public let recent_transactions: [HistoryTransactionV1]
}

public struct HistoryVerificationV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let total_records: Int
    public let is_valid: Bool
    public let first_hash: String?
    public let latest_hash: String?
    public let broken_record_id: Int64?
    public let error_detail: String?
}

// MARK: - Formatting & Display Helpers

public enum Formatters {
    public static func formatBytes(_ bytes: UInt64) -> String {
        Int64(bytes).formatted(.byteCount(style: .file))
    }

    public static func formatBytesSigned(_ bytes: Int64) -> String {
        let prefix = bytes > 0 ? "+" : ""
        return prefix + bytes.formatted(.byteCount(style: .file))
    }
}

// MARK: - Storage Tree Models

public struct StorageTreeCoverageV1: Codable, Sendable, Equatable {
    public let files_observed: UInt64
    public let directories_observed: UInt64
    public let entries_skipped: UInt64
    public let permission_errors: UInt64
    public let mount_boundary_skips: UInt64
    public let special_files_skipped: UInt64
    public let cloud_placeholders_observed: UInt64
    public let analysis_complete: Bool
}

public struct StorageTreeNodeV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { node_id }

    public let node_id: String
    public let parent_node_id: String?
    public let display_name: String
    public let display_path: String
    public let kind: String
    public let depth: UInt32
    public let direct_logical_bytes: UInt64
    public let direct_allocated_bytes: UInt64
    public let subtree_logical_bytes: UInt64
    public let subtree_allocated_bytes: UInt64
    public let file_count: UInt64
    public let directory_count: UInt64
    public let hardlink_alias_count: UInt64
    public let is_hardlink_alias: Bool
    public let child_count: UInt64
    public let mtime_sec: Int64

    public var isDirectory: Bool {
        kind == "directory" || kind == "root"
    }
}

public struct StorageTreeRemainderV1: Codable, Sendable, Equatable {
    public let item_count: UInt64
    public let logical_bytes: UInt64
    public let allocated_bytes: UInt64
}

public struct StorageTreeAnalysisV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { generation_id }

    public let schema_version: String
    public let generation_id: String
    public let root_path: String
    public let root_id: String
    public let observed_at: String
    public let source: String
    public let root_node: StorageTreeNodeV1
    public let total_files: UInt64
    public let total_dirs: UInt64
    public let total_logical_bytes: UInt64
    public let total_allocated_bytes: UInt64
    public let physical_sharing_uncertainty: Bool
    public let allocation_semantics: String
    public let coverage: StorageTreeCoverageV1
}

public struct StorageTreePageV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let generation_id: String
    public let parent_node: StorageTreeNodeV1
    public let metric: String
    public let items: [StorageTreeNodeV1]
    public let total_child_count: Int
    public let limit: Int
    public let offset: Int
    public let remainder: StorageTreeRemainderV1
    public let item_deltas: [StorageTreeDeltaV1]?
    public let next_cursor: String?
}

public struct StorageTreeDeltaV1: Codable, Sendable, Identifiable, Equatable {
    public var id: String { node_id }

    public let node_id: String
    public let allocated_delta_bytes: Int64
    public let logical_delta_bytes: Int64
    public let file_count_delta: Int64
    public let change_kind: String
}

public struct StorageTreeNodeDetailV1: Codable, Sendable, Equatable {
    public let schema_version: String
    public let node: StorageTreeNodeV1
    public let percentage_of_parent: Double?
    public let percentage_of_root: Double
    public let hardlink_info: String?
    public let allocation_semantics: String
    public let delta: StorageTreeDeltaV1?
}

