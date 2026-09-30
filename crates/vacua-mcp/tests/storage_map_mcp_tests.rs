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
