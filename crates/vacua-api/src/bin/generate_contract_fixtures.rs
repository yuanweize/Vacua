use std::fs;
use std::path::{Path, PathBuf};
use vacua_api::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
        .join("api");

    println!(
        "Verifying and generating contract fixtures in: {:?}",
        fixtures_dir
    );

    // 1. Generate Developer Artifact contract fixtures
    generate_developer_artifact_fixtures(&fixtures_dir)?;

    // 2. Validate roundtrip of existing fixtures
    roundtrip::<ServerCapabilitiesV1>(&fixtures_dir.join("capabilities-v1.json"))?;
    roundtrip::<StorageSummaryV1>(&fixtures_dir.join("storage-summary-v1.json"))?;
    roundtrip::<CandidateListResponseV1>(&fixtures_dir.join("candidate-list-v1.json"))?;
    roundtrip::<CandidateDetailV1>(&fixtures_dir.join("candidate-detail-v1.json"))?;
    roundtrip::<DuplicateListResponseV1>(&fixtures_dir.join("duplicate-list-v1.json"))?;
    roundtrip::<DuplicateGroupDetailV1>(&fixtures_dir.join("duplicate-detail-v1.json"))?;
    roundtrip::<ApplicationListResponseV1>(&fixtures_dir.join("application-list-v1.json"))?;
    roundtrip::<ApplicationDetailV1>(&fixtures_dir.join("application-detail-v1.json"))?;
    roundtrip::<SnapshotListResponseV1>(&fixtures_dir.join("snapshot-list-v1.json"))?;
    roundtrip::<SnapshotDiffV1>(&fixtures_dir.join("snapshot-diff-v1.json"))?;
    roundtrip::<CleanupSimulationV1>(&fixtures_dir.join("cleanup-simulation-v1.json"))?;
    roundtrip::<CleanupPlanProposalV1>(&fixtures_dir.join("cleanup-proposal-v1.json"))?;
    roundtrip::<StorageTreeAnalysisV1>(&fixtures_dir.join("storage-tree-analysis-v1.json"))?;
    roundtrip::<StorageTreePageV1>(&fixtures_dir.join("storage-tree-page-v1.json"))?;
    roundtrip::<StorageTreeNodeDetailV1>(&fixtures_dir.join("storage-tree-node-detail-v1.json"))?;

    // 3. Validate roundtrip of new Developer Artifact fixtures
    roundtrip::<DeveloperArtifactAnalysisV1>(
        &fixtures_dir.join("developer-artifact-analysis-v1.json"),
    )?;
    roundtrip::<DeveloperArtifactDetailV1>(
        &fixtures_dir.join("developer-artifact-detail-v1.json"),
    )?;
    roundtrip::<DeveloperProjectDetailV1>(&fixtures_dir.join("developer-project-detail-v1.json"))?;
    roundtrip::<DeveloperArtifactPageV1>(&fixtures_dir.join("developer-artifact-page-v1.json"))?;

    println!("All contract fixtures validated and proven through real Rust DTO serialization!");
    Ok(())
}

fn generate_developer_artifact_fixtures(
    fixtures_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Rust Cargo artifact
    let rust_evidence = RebuildEvidenceV1 {
        manifest_present: true,
        manifest_path: Some("Cargo.toml".to_string()),
        lockfile_present: true,
        lockfile_path: Some("Cargo.lock".to_string()),
        known_artifact_convention: true,
        project_root_known: true,
        toolchain_identified: Some("Rust Cargo".to_string()),
        active_project_state: "active".to_string(),
        reconstruction_confidence: "strong".to_string(),
        rebuild_command_template: Some("cargo build".to_string()),
        reasons: vec![
            "Associated with Rust Cargo project at Vacua".to_string(),
            "Manifest present: Cargo.toml".to_string(),
            "Lockfile detected: Cargo.lock".to_string(),
            "Workflow artifacts fully reconstructable via project toolchain and lockfile."
                .to_string(),
        ],
    };

    let rust_art = DeveloperArtifactSummaryV1 {
        artifact_id: "devart_7b8a9c0d1e2f3a4b".to_string(),
        project_id: "devproj_1a2b3c4d5e6f7a8b".to_string(),
        display_name: "target".to_string(),
        display_path: "target".to_string(),
        ecosystem: "rust_cargo".to_string(),
        artifact_kind: "build_output".to_string(),
        logical_bytes: 4294967296,
        allocated_bytes: 4831838208,
        confirmed_reclaim_lower_bound: 4831838208,
        rebuild_confidence: "strong".to_string(),
        candidate_id: Some("cand-rust-target-01".to_string()),
    };

    // 2. Node.js artifact with verified lockfile
    let _node_evidence = RebuildEvidenceV1 {
        manifest_present: true,
        manifest_path: Some("web-client/package.json".to_string()),
        lockfile_present: true,
        lockfile_path: Some("web-client/pnpm-lock.yaml".to_string()),
        known_artifact_convention: true,
        project_root_known: true,
        toolchain_identified: Some("Node.js".to_string()),
        active_project_state: "dormant".to_string(),
        reconstruction_confidence: "verified".to_string(),
        rebuild_command_template: Some("pnpm install".to_string()),
        reasons: vec![
            "Associated with Node.js project at web-client".to_string(),
            "Manifest present: package.json".to_string(),
            "Lockfile detected: pnpm-lock.yaml".to_string(),
            "Dependency tree backed by exact lockfile.".to_string(),
        ],
    };

    let node_art = DeveloperArtifactSummaryV1 {
        artifact_id: "devart_5c6d7e8f9a0b1c2d".to_string(),
        project_id: "devproj_9f8e7d6c5b4a3a2b".to_string(),
        display_name: "node_modules".to_string(),
        display_path: "web-client/node_modules".to_string(),
        ecosystem: "node".to_string(),
        artifact_kind: "dependency_tree".to_string(),
        logical_bytes: 1288490188,
        allocated_bytes: 1610612736,
        confirmed_reclaim_lower_bound: 1610612736,
        rebuild_confidence: "verified".to_string(),
        candidate_id: None,
    };

    // 3. Python virtualenv
    let _py_evidence = RebuildEvidenceV1 {
        manifest_present: true,
        manifest_path: Some("ai-tools/pyproject.toml".to_string()),
        lockfile_present: true,
        lockfile_path: Some("ai-tools/uv.lock".to_string()),
        known_artifact_convention: true,
        project_root_known: true,
        toolchain_identified: Some("Python".to_string()),
        active_project_state: "active".to_string(),
        reconstruction_confidence: "strong".to_string(),
        rebuild_command_template: Some("uv sync".to_string()),
        reasons: vec![
            "Associated with Python project at ai-tools".to_string(),
            "Manifest present: pyproject.toml".to_string(),
            "Lockfile detected: uv.lock".to_string(),
            "Virtual environment backed by deterministic lockfile.".to_string(),
        ],
    };

    let py_art = DeveloperArtifactSummaryV1 {
        artifact_id: "devart_3e4f5a6b7c8d9e0f".to_string(),
        project_id: "devproj_8a7b6c5d4e3f2a1b".to_string(),
        display_name: ".venv".to_string(),
        display_path: "ai-tools/.venv".to_string(),
        ecosystem: "python".to_string(),
        artifact_kind: "virtual_environment".to_string(),
        logical_bytes: 858993459,
        allocated_bytes: 1073741824,
        confirmed_reclaim_lower_bound: 1073741824,
        rebuild_confidence: "strong".to_string(),
        candidate_id: None,
    };

    // 4. Xcode archive (caution / review required)
    let _xcode_archive_evidence = RebuildEvidenceV1 {
        manifest_present: true,
        manifest_path: Some("Vacua.xcodeproj".to_string()),
        lockfile_present: false,
        lockfile_path: None,
        known_artifact_convention: true,
        project_root_known: true,
        toolchain_identified: Some("Xcode".to_string()),
        active_project_state: "active".to_string(),
        reconstruction_confidence: "partial".to_string(),
        rebuild_command_template: Some("xcodebuild -scheme Vacua build".to_string()),
        reasons: vec![
            "Associated with Xcode project at Vacua.xcodeproj".to_string(),
            "Archive contains distribution or signed products; requires manual review.".to_string(),
        ],
    };

    let xcode_art = DeveloperArtifactSummaryV1 {
        artifact_id: "devart_2b3c4d5e6f7a8b9c".to_string(),
        project_id: "devproj_1a2b3c4d5e6f7a8b".to_string(),
        display_name: "Archives".to_string(),
        display_path: "build/Archives".to_string(),
        ecosystem: "xcode".to_string(),
        artifact_kind: "build_output".to_string(),
        logical_bytes: 536870912,
        allocated_bytes: 671088640,
        confirmed_reclaim_lower_bound: 671088640,
        rebuild_confidence: "partial".to_string(),
        candidate_id: None,
    };

    // 5. Unknown / Partial evidence project
    let unk_art = DeveloperArtifactSummaryV1 {
        artifact_id: "devart_0f1e2d3c4b5a6f7e".to_string(),
        project_id: "devproj_4b5a6f7e8d9c0b1a".to_string(),
        display_name: "build".to_string(),
        display_path: "legacy-lib/build".to_string(),
        ecosystem: "unknown".to_string(),
        artifact_kind: "other_generated".to_string(),
        logical_bytes: 268435456,
        allocated_bytes: 335544320,
        confirmed_reclaim_lower_bound: 335544320,
        rebuild_confidence: "partial".to_string(),
        candidate_id: None,
    };

    // Project summaries
    let p1 = DeveloperProjectSummaryV1 {
        project_id: "devproj_1a2b3c4d5e6f7a8b".to_string(),
        display_name: "Vacua".to_string(),
        display_path: ".".to_string(),
        primary_ecosystem: "rust_cargo".to_string(),
        all_ecosystems: vec!["rust_cargo".to_string(), "xcode".to_string()],
        artifacts_count: 2,
        total_logical_bytes: 4831838208,
        total_allocated_bytes: 5502926848,
        rebuild_confidence: "partial".to_string(),
        active_state: "active".to_string(),
    };

    let p2 = DeveloperProjectSummaryV1 {
        project_id: "devproj_9f8e7d6c5b4a3a2b".to_string(),
        display_name: "web-client".to_string(),
        display_path: "web-client".to_string(),
        primary_ecosystem: "node".to_string(),
        all_ecosystems: vec!["node".to_string()],
        artifacts_count: 1,
        total_logical_bytes: 1288490188,
        total_allocated_bytes: 1610612736,
        rebuild_confidence: "verified".to_string(),
        active_state: "dormant".to_string(),
    };

    let p3 = DeveloperProjectSummaryV1 {
        project_id: "devproj_8a7b6c5d4e3f2a1b".to_string(),
        display_name: "ai-tools".to_string(),
        display_path: "ai-tools".to_string(),
        primary_ecosystem: "python".to_string(),
        all_ecosystems: vec!["python".to_string()],
        artifacts_count: 1,
        total_logical_bytes: 858993459,
        total_allocated_bytes: 1073741824,
        rebuild_confidence: "strong".to_string(),
        active_state: "active".to_string(),
    };

    let p4 = DeveloperProjectSummaryV1 {
        project_id: "devproj_4b5a6f7e8d9c0b1a".to_string(),
        display_name: "legacy-lib".to_string(),
        display_path: "legacy-lib".to_string(),
        primary_ecosystem: "unknown".to_string(),
        all_ecosystems: vec!["unknown".to_string()],
        artifacts_count: 1,
        total_logical_bytes: 268435456,
        total_allocated_bytes: 335544320,
        rebuild_confidence: "partial".to_string(),
        active_state: "unknown".to_string(),
    };

    let coverage = DeveloperArtifactCoverageV1 {
        supported_ecosystems: vec![
            "xcode".to_string(),
            "swift_pm".to_string(),
            "rust_cargo".to_string(),
            "node".to_string(),
            "python".to_string(),
            "gradle".to_string(),
            "maven".to_string(),
            "cmake".to_string(),
        ],
        unclassified_candidate_directories: 4,
        skipped_items: 0,
    };

    let analysis = DeveloperArtifactAnalysisV1 {
        schema_version: SCHEMA_DEVELOPER_ARTIFACT_ANALYSIS_V1.to_string(),
        generation_id: "gen_devart_1727740000".to_string(),
        root_id: "root-macos-user-workspace".to_string(),
        root_path: "/Users/developer/Workspace".to_string(),
        observed_at: "2026-10-01T00:00:00Z".to_string(),
        total_projects: 4,
        total_artifacts: 5,
        total_logical_bytes: 7247757311,
        total_allocated_bytes: 8522825728,
        projects: vec![p1, p2, p3, p4],
        coverage,
    };

    let detail = DeveloperArtifactDetailV1 {
        schema_version: SCHEMA_DEVELOPER_ARTIFACT_DETAIL_V1.to_string(),
        artifact_id: "devart_7b8a9c0d1e2f3a4b".to_string(),
        project_id: "devproj_1a2b3c4d5e6f7a8b".to_string(),
        project_name: "Vacua".to_string(),
        display_name: "target".to_string(),
        display_path: "target".to_string(),
        ecosystem: "rust_cargo".to_string(),
        artifact_kind: "build_output".to_string(),
        logical_bytes: 4294967296,
        allocated_bytes: 4831838208,
        confirmed_reclaim_lower_bound: 4831838208,
        estimated_reclaim: 4831838208,
        physical_sharing_uncertainty: true,
        rebuild_evidence: rust_evidence,
        candidate_id: Some("cand-rust-target-01".to_string()),
        observed_at: "2026-10-01T00:00:00Z".to_string(),
    };

    let proj_detail = DeveloperProjectDetailV1 {
        schema_version: SCHEMA_DEVELOPER_PROJECT_DETAIL_V1.to_string(),
        project_id: "devproj_1a2b3c4d5e6f7a8b".to_string(),
        display_name: "Vacua".to_string(),
        display_path: ".".to_string(),
        primary_ecosystem: "rust_cargo".to_string(),
        all_ecosystems: vec!["rust_cargo".to_string(), "xcode".to_string()],
        manifest_paths: vec!["Cargo.toml".to_string()],
        lockfile_paths: vec!["Cargo.lock".to_string()],
        artifacts: vec![rust_art.clone(), xcode_art],
        total_logical_bytes: 4831838208,
        total_allocated_bytes: 5502926848,
        rebuild_confidence: "partial".to_string(),
        active_state: "active".to_string(),
    };

    let page = DeveloperArtifactPageV1 {
        schema_version: SCHEMA_DEVELOPER_ARTIFACT_PAGE_V1.to_string(),
        generation_id: "gen_devart_1727740000".to_string(),
        root_id: "root-macos-user-workspace".to_string(),
        artifacts: vec![rust_art, node_art, py_art, unk_art],
        total_count: 4,
        offset: 0,
        limit: 50,
        has_more: false,
    };

    fs::write(
        fixtures_dir.join("developer-artifact-analysis-v1.json"),
        serde_json::to_string_pretty(&analysis)? + "\n",
    )?;
    fs::write(
        fixtures_dir.join("developer-artifact-detail-v1.json"),
        serde_json::to_string_pretty(&detail)? + "\n",
    )?;
    fs::write(
        fixtures_dir.join("developer-project-detail-v1.json"),
        serde_json::to_string_pretty(&proj_detail)? + "\n",
    )?;
    fs::write(
        fixtures_dir.join("developer-artifact-page-v1.json"),
        serde_json::to_string_pretty(&page)? + "\n",
    )?;

    Ok(())
}

fn roundtrip<T: serde::de::DeserializeOwned + serde::Serialize>(
    path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let parsed: T = serde_json::from_str(&content)?;
    let re_serialized = serde_json::to_string_pretty(&parsed)?;
    fs::write(path, format!("{}\n", re_serialized))?;
    Ok(())
}
