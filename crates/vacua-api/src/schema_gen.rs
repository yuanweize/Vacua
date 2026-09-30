use schemars::schema_for;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::dto::*;

/// Generate a map of filename to JSON Schema value for all public DTOs.
pub fn generate_all_schemas() -> BTreeMap<&'static str, Value> {
    let mut map = BTreeMap::new();

    map.insert(
        "storage-summary-v1.schema.json",
        serde_json::to_value(schema_for!(StorageSummaryV1)).unwrap(),
    );
    map.insert(
        "candidate-summary-v1.schema.json",
        serde_json::to_value(schema_for!(CandidateSummaryV1)).unwrap(),
    );
    map.insert(
        "candidate-list-v1.schema.json",
        serde_json::to_value(schema_for!(CandidateListResponseV1)).unwrap(),
    );
    map.insert(
        "candidate-detail-v1.schema.json",
        serde_json::to_value(schema_for!(CandidateDetailV1)).unwrap(),
    );
    map.insert(
        "snapshot-summary-v1.schema.json",
        serde_json::to_value(schema_for!(SnapshotSummaryV1)).unwrap(),
    );
    map.insert(
        "snapshot-list-v1.schema.json",
        serde_json::to_value(schema_for!(SnapshotListResponseV1)).unwrap(),
    );
    map.insert(
        "snapshot-detail-v1.schema.json",
        serde_json::to_value(schema_for!(SnapshotDetailV1)).unwrap(),
    );
    map.insert(
        "snapshot-diff-v1.schema.json",
        serde_json::to_value(schema_for!(SnapshotDiffV1)).unwrap(),
    );
    map.insert(
        "application-summary-v1.schema.json",
        serde_json::to_value(schema_for!(ApplicationSummaryV1)).unwrap(),
    );
    map.insert(
        "application-list-v1.schema.json",
        serde_json::to_value(schema_for!(ApplicationListResponseV1)).unwrap(),
    );
    map.insert(
        "application-detail-v1.schema.json",
        serde_json::to_value(schema_for!(ApplicationDetailV1)).unwrap(),
    );
    map.insert(
        "duplicate-group-v1.schema.json",
        serde_json::to_value(schema_for!(DuplicateGroupSummaryV1)).unwrap(),
    );
    map.insert(
        "duplicate-list-v1.schema.json",
        serde_json::to_value(schema_for!(DuplicateListResponseV1)).unwrap(),
    );
    map.insert(
        "duplicate-detail-v1.schema.json",
        serde_json::to_value(schema_for!(DuplicateGroupDetailV1)).unwrap(),
    );
    map.insert(
        "cleanup-simulation-v1.schema.json",
        serde_json::to_value(schema_for!(CleanupSimulationV1)).unwrap(),
    );
    map.insert(
        "plan-proposal-v1.schema.json",
        serde_json::to_value(schema_for!(CleanupPlanProposalV1)).unwrap(),
    );
    map.insert(
        "server-capabilities-v1.schema.json",
        serde_json::to_value(schema_for!(ServerCapabilitiesV1)).unwrap(),
    );
    map.insert(
        "history-summary-v1.schema.json",
        serde_json::to_value(schema_for!(HistorySummaryV1)).unwrap(),
    );
    map.insert(
        "history-verification-v1.schema.json",
        serde_json::to_value(schema_for!(HistoryVerificationV1)).unwrap(),
    );
    map.insert(
        "build-info-v1.schema.json",
        serde_json::to_value(schema_for!(BuildInfoV1)).unwrap(),
    );
    map.insert(
        "release-manifest-v1.schema.json",
        serde_json::to_value(schema_for!(ReleaseManifestV1)).unwrap(),
    );

    map
}
