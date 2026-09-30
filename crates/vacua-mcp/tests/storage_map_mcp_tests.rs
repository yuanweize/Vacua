use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use vacua_mcp::domain::VacuaDomainService;
use vacua_mcp::policy::{AllowedRoot, McpPolicy, PathDisclosureMode};

fn setup_test_service(temp_dir: &TempDir) -> (VacuaDomainService, PathBuf) {
    let root_path = temp_dir.path().join("test_home");
    fs::create_dir_all(root_path.join("Documents/Projects")).unwrap();
    fs::create_dir_all(root_path.join("Library/Caches")).unwrap();
    fs::write(root_path.join("Documents/file1.txt"), "hello world").unwrap();
    fs::write(
        root_path.join("Documents/Projects/proj.dat"),
        vec![0u8; 1024],
    )
    .unwrap();

    let canonical_root = root_path.canonicalize().unwrap();
    let allowed_root = AllowedRoot {
        root_id: "root-home".to_string(),
        canonical_path: canonical_root.clone(),
        display_name: "~/".to_string(),
    };

    let policy = McpPolicy::new(
        vec![allowed_root],
        PathDisclosureMode::HomeRelative,
        50,
        2,
        std::time::Duration::from_secs(30),
        false,
        false,
    );

    let index_path = temp_dir.path().join("index.db");
    let service = VacuaDomainService::new(policy, Some(index_path), None);
    (service, root_path)
}

#[tokio::test]
async fn test_analyze_and_query_storage_map_mcp() {
    let temp_dir = TempDir::new().unwrap();
    let (service, _root_path) = setup_test_service(&temp_dir);

    // 1. Analyze storage map
    let analysis = service
        .analyze_storage_map(Some("root-home"), Some(true))
        .await
        .expect("Analyze storage map should succeed");

    assert_eq!(analysis.root_id, "root-home");
    assert!(!analysis.generation_id.is_empty());
    assert!(analysis.total_files >= 2);
    assert!(analysis.total_dirs >= 3);
    assert!(analysis.physical_sharing_uncertainty);
    assert!(analysis.allocation_semantics.contains("APFS clone"));

    // 2. Query storage map children of root
    let page = service
        .get_storage_map(
            "root-home",
            &analysis.generation_id,
            None,
            Some("allocated"),
            Some(10),
            Some(0),
            None,
        )
        .expect("Query root children page should succeed");

    assert_eq!(page.generation_id, analysis.generation_id);
    assert!(!page.items.is_empty());
    assert_eq!(
        page.items.len() + page.remainder.item_count as usize,
        page.total_child_count
    );

    // Verify node IDs are opaque
    for item in &page.items {
        assert!(
            item.node_id.starts_with("stn_"),
            "Node ID must be opaque stn_ prefix: {}",
            item.node_id
        );
        assert!(
            !item.node_id.contains('/'),
            "Node ID must never contain raw slashes"
        );
    }

    // 3. Inspect a single node
    let first_child = &page.items[0];
    let detail = service
        .get_storage_node(
            "root-home",
            &analysis.generation_id,
            &first_child.node_id,
            None,
        )
        .expect("Query node detail should succeed");

    assert_eq!(detail.node.node_id, first_child.node_id);
    assert!(detail.percentage_of_parent.is_some());
    assert!(detail.allocation_semantics.contains("APFS clone"));

    // 4. Stale generation returns error
    let stale_err = service.get_storage_map(
        "root-home",
        "stg_nonexistent_12345",
        None,
        Some("allocated"),
        Some(10),
        Some(0),
        None,
    );
    assert!(stale_err.is_err());
    let err = stale_err.unwrap_err();
    assert_eq!(err.code, vacua_api::VacuaErrorCode::VacuaStaleState);
}

#[tokio::test]
async fn test_unauthorized_root_denied() {
    let temp_dir = TempDir::new().unwrap();
    let (service, _root_path) = setup_test_service(&temp_dir);

    let err = service
        .analyze_storage_map(Some("malicious-root"), Some(false))
        .await
        .unwrap_err();
    assert_eq!(err.code, vacua_api::VacuaErrorCode::VacuaNotFound);
}

#[tokio::test]
async fn test_generational_pruning_keeps_latest_two() {
    let temp_dir = TempDir::new().unwrap();
    let (service, _root_path) = setup_test_service(&temp_dir);

    let a1 = service
        .analyze_storage_map(Some("root-home"), Some(true))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let a2 = service
        .analyze_storage_map(Some("root-home"), Some(true))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let a3 = service
        .analyze_storage_map(Some("root-home"), Some(true))
        .await
        .unwrap();

    // Latest 2 (a2, a3) should be queryable, oldest (a1) pruned
    let q3 = service.get_storage_map(
        "root-home",
        &a3.generation_id,
        None,
        None,
        Some(10),
        Some(0),
        None,
    );
    assert!(
        q3.is_ok(),
        "Latest generation a3 should be accessible: {:?}",
        q3.err()
    );

    let q2 = service.get_storage_map(
        "root-home",
        &a2.generation_id,
        None,
        None,
        Some(10),
        Some(0),
        None,
    );
    assert!(
        q2.is_ok(),
        "Second latest generation a2 should be accessible: {:?}",
        q2.err()
    );

    let q1 = service.get_storage_map(
        "root-home",
        &a1.generation_id,
        None,
        None,
        Some(10),
        Some(0),
        None,
    );
    assert!(q1.is_err(), "Oldest generation a1 should be pruned");
    assert_eq!(
        q1.unwrap_err().code,
        vacua_api::VacuaErrorCode::VacuaStaleState
    );
}

#[test]
fn test_vacua_mcp_dependency_boundary_storage_tree() {
    let output = std::process::Command::new("cargo")
        .args(["tree", "-p", "vacua-mcp", "--prefix", "none"])
        .output()
        .expect("Failed to run cargo tree");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("vacua-executor"),
        "CRITICAL: vacua-mcp must NEVER depend on vacua-executor"
    );
}

#[tokio::test]
async fn test_storage_map_page_snapshot_deltas() {
    let temp_dir = TempDir::new().unwrap();
    let root_path = temp_dir.path().join("test_home");
    let dir_a = root_path.join("folderA");
    let dir_b = root_path.join("folderB");
    let dir_c = root_path.join("folderC");
    let dir_d = root_path.join("folderD");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(&dir_b).unwrap();
    fs::create_dir_all(&dir_c).unwrap();
    fs::create_dir_all(&dir_d).unwrap();

    // folderA: 3000 bytes (will grow compared to baseline 1000)
    fs::write(dir_a.join("fileA.dat"), vec![0u8; 3000]).unwrap();
    // folderB: 500 bytes (will shrink compared to baseline 5000)
    fs::write(dir_b.join("fileB.dat"), vec![0u8; 500]).unwrap();
    // folderC: 2000 bytes (unchanged compared to baseline 2000)
    fs::write(dir_c.join("fileC.dat"), vec![0u8; 2000]).unwrap();
    // folderD: 1000 bytes (new, not in baseline)
    fs::write(dir_d.join("fileD.dat"), vec![0u8; 1000]).unwrap();

    let canonical_root = root_path.canonicalize().unwrap();
    let allowed_root = AllowedRoot {
        root_id: "root-home".to_string(),
        canonical_path: canonical_root.clone(),
        display_name: "~/".to_string(),
    };

    let policy = McpPolicy::new(
        vec![allowed_root],
        PathDisclosureMode::HomeRelative,
        50,
        2,
        std::time::Duration::from_secs(30),
        false,
        false,
    );

    let index_path = temp_dir.path().join("index.db");

    // Write baseline snapshot into index
    {
        let mut db = vacua_index::IndexDatabase::open(&index_path).expect("open index db");
        let mut subtrees = std::collections::HashMap::new();
        subtrees.insert(
            "folderA".to_string(),
            vacua_index::SnapshotSubtreeStats {
                allocated_bytes: 4096,
                logical_bytes: 1000,
                file_count: 1,
                dir_count: 1,
            },
        );
        subtrees.insert(
            "folderB".to_string(),
            vacua_index::SnapshotSubtreeStats {
                allocated_bytes: 8192,
                logical_bytes: 5000,
                file_count: 2,
                dir_count: 1,
            },
        );
        subtrees.insert(
            "folderC".to_string(),
            vacua_index::SnapshotSubtreeStats {
                allocated_bytes: 4096,
                logical_bytes: 2096,
                file_count: 1,
                dir_count: 1,
            },
        );
        // folderD is NOT in subtrees

        let snapshot = vacua_index::StorageSnapshot {
            snapshot_id: "snap-baseline-1".to_string(),
            name: "Baseline".to_string(),
            root_path: canonical_root.clone(),
            timestamp: 100,
            total_files: 4,
            total_dirs: 3,
            logical_bytes: 8000,
            allocated_bytes: 16384,
            subtrees,
        };
        db.save_snapshot(&snapshot).expect("save snapshot");
    }

    let service = VacuaDomainService::new(policy, Some(index_path), None);

    let analysis = service
        .analyze_storage_map(Some("root-home"), Some(true))
        .await
        .expect("analyze storage map");

    let page = service
        .get_storage_map(
            "root-home",
            &analysis.generation_id,
            None,
            Some("allocated"),
            Some(10),
            Some(0),
            Some("snap-baseline-1"),
        )
        .expect("get_storage_map with snapshot");

    assert!(page.item_deltas.is_some(), "item_deltas must be present");
    let deltas = page.item_deltas.unwrap();
    assert_eq!(deltas.len(), page.items.len());

    let mut kinds_by_name = std::collections::HashMap::new();
    for (item, delta) in page.items.iter().zip(deltas.iter()) {
        assert_eq!(item.node_id, delta.node_id);
        kinds_by_name.insert(item.display_name.clone(), delta.change_kind.clone());
    }

    assert_eq!(
        kinds_by_name.get("folderA").map(|s| s.as_str()),
        Some("grown")
    );
    assert_eq!(
        kinds_by_name.get("folderB").map(|s| s.as_str()),
        Some("shrunk")
    );
    assert_eq!(
        kinds_by_name.get("folderC").map(|s| s.as_str()),
        Some("unchanged")
    );
    assert_eq!(
        kinds_by_name.get("folderD").map(|s| s.as_str()),
        Some("new")
    );
}
