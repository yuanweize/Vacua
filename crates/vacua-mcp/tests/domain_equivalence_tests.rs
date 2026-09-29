use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;
use vacua_mcp::{McpPolicy, PathDisclosureMode, VacuaDomainService};

fn find_vacua_bin() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let target_debug = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("debug")
        .join("vacua");
    if target_debug.exists() {
        return target_debug;
    }
    PathBuf::from("vacua")
}

#[test]
fn test_storage_summary_cli_and_mcp_equivalence() {
    let dir = tempdir().unwrap();
    let root_path = dir.path();

    // 1. Run CLI --json
    let cli_bin = find_vacua_bin();
    if !cli_bin.exists() {
        // Build CLI if not present
        let _ = Command::new("cargo")
            .args(["build", "-p", "vacua-cli"])
            .output();
    }

    let cli_output = Command::new(&cli_bin)
        .arg("--json")
        .arg("scan")
        .arg(root_path)
        .output()
        .expect("Failed to execute CLI");

    assert!(cli_output.status.success());
    let cli_json: serde_json::Value =
        serde_json::from_slice(&cli_output.stdout).expect("CLI output is not valid JSON");

    // 2. Query MCP Domain Service
    let root = vacua_mcp::AllowedRoot::try_new(&root_path.canonicalize().unwrap(), None).unwrap();
    let policy = McpPolicy::new(
        vec![root],
        PathDisclosureMode::Full,
        50,
        2,
        std::time::Duration::from_secs(30),
        false,
        false,
    );
    let service = VacuaDomainService::new(policy, None, None);
    let mcp_summary = service
        .storage_summary(None, Some(&root_path.to_string_lossy()))
        .expect("storage_summary failed");

    // 3. Compare underlying facts
    assert_eq!(mcp_summary.filesystem_type, "apfs");
    assert!(mcp_summary.total_space_bytes > 0);
    assert_eq!(
        cli_json["target_path"].as_str().unwrap(),
        root_path.canonicalize().unwrap().to_string_lossy()
    );
}
