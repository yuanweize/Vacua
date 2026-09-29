use std::fs;
use std::path::PathBuf;
use vacua_api::*;

#[test]
fn test_contract_fixtures_deserialize_cleanly() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let fixtures_dir = workspace_root.join("fixtures").join("api");

    // 1. Capabilities
    let caps_json = fs::read_to_string(fixtures_dir.join("capabilities-v1.json")).unwrap();
    let caps: ServerCapabilitiesV1 = serde_json::from_str(&caps_json).unwrap();
    assert_eq!(caps.schema_version, SCHEMA_SERVER_CAPABILITIES_V1);
    assert!(!caps.mutation_authority);
    assert!(!caps.executor_linked);

    // 2. Storage Summary
    let stor_json = fs::read_to_string(fixtures_dir.join("storage-summary-v1.json")).unwrap();
    let stor: StorageSummaryV1 = serde_json::from_str(&stor_json).unwrap();
    assert_eq!(stor.schema_version, SCHEMA_STORAGE_SUMMARY_V1);
    assert_eq!(stor.filesystem_type, "apfs");
    assert!(stor.candidate_confirmed_reclaim_bytes > 0);

    // 3. Candidate List
    let cand_list_json = fs::read_to_string(fixtures_dir.join("candidate-list-v1.json")).unwrap();
    let cand_list: CandidateListResponseV1 = serde_json::from_str(&cand_list_json).unwrap();
    assert_eq!(cand_list.schema_version, SCHEMA_CANDIDATE_LIST_V1);
    assert_eq!(cand_list.items.len(), 2);

    // 4. Candidate Detail
    let cand_det_json = fs::read_to_string(fixtures_dir.join("candidate-detail-v1.json")).unwrap();
    let cand_det: CandidateDetailV1 = serde_json::from_str(&cand_det_json).unwrap();
    assert_eq!(cand_det.schema_version, SCHEMA_CANDIDATE_DETAIL_V1);
    assert_eq!(cand_det.risk, "safe");

    // 5. Duplicate List
    let dup_list_json = fs::read_to_string(fixtures_dir.join("duplicate-list-v1.json")).unwrap();
    let dup_list: DuplicateListResponseV1 = serde_json::from_str(&dup_list_json).unwrap();
    assert_eq!(dup_list.schema_version, SCHEMA_DUPLICATE_LIST_V1);
    assert_eq!(dup_list.items.len(), 1);

    // 6. Duplicate Detail
    let dup_det_json = fs::read_to_string(fixtures_dir.join("duplicate-detail-v1.json")).unwrap();
    let dup_det: DuplicateGroupDetailV1 = serde_json::from_str(&dup_det_json).unwrap();
    assert_eq!(dup_det.schema_version, SCHEMA_DUPLICATE_GROUP_V1);
    assert_eq!(dup_det.members.len(), 3);

    // 7. Application List
    let app_list_json = fs::read_to_string(fixtures_dir.join("application-list-v1.json")).unwrap();
    let app_list: ApplicationListResponseV1 = serde_json::from_str(&app_list_json).unwrap();
    assert_eq!(app_list.schema_version, SCHEMA_APPLICATION_LIST_V1);
    assert_eq!(app_list.items.len(), 2);

    // 8. Application Detail
    let app_det_json = fs::read_to_string(fixtures_dir.join("application-detail-v1.json")).unwrap();
    let app_det: ApplicationDetailV1 = serde_json::from_str(&app_det_json).unwrap();
    assert_eq!(app_det.schema_version, SCHEMA_APPLICATION_DETAIL_V1);
    assert_eq!(app_det.artifacts.len(), 2);

    // 9. Snapshot List
    let snap_list_json = fs::read_to_string(fixtures_dir.join("snapshot-list-v1.json")).unwrap();
    let snap_list: SnapshotListResponseV1 = serde_json::from_str(&snap_list_json).unwrap();
    assert_eq!(snap_list.schema_version, SCHEMA_SNAPSHOT_LIST_V1);
    assert_eq!(snap_list.items.len(), 1);

    // 10. Snapshot Diff
    let snap_diff_json = fs::read_to_string(fixtures_dir.join("snapshot-diff-v1.json")).unwrap();
    let snap_diff: SnapshotDiffV1 = serde_json::from_str(&snap_diff_json).unwrap();
    assert_eq!(snap_diff.schema_version, SCHEMA_SNAPSHOT_DIFF_V1);
    assert_eq!(snap_diff.top_growing.len(), 1);

    // 11. Cleanup Simulation
    let sim_json = fs::read_to_string(fixtures_dir.join("cleanup-simulation-v1.json")).unwrap();
    let sim: CleanupSimulationV1 = serde_json::from_str(&sim_json).unwrap();
    assert_eq!(sim.schema_version, SCHEMA_CLEANUP_SIMULATION_V1);
    assert_eq!(sim.immediate_reclaim_bytes, 0);

    // 12. Cleanup Proposal
    let prop_json = fs::read_to_string(fixtures_dir.join("cleanup-proposal-v1.json")).unwrap();
    let prop: CleanupPlanProposalV1 = serde_json::from_str(&prop_json).unwrap();
    assert_eq!(prop.schema_version, SCHEMA_PLAN_PROPOSAL_V1);
    assert_eq!(prop.proposal_status, "PROPOSAL_ONLY_NOT_EXECUTABLE_VIA_MCP");
    assert!(prop.serialized_plan.is_none());
}
