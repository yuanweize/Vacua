use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

const DESTRUCTIVE_NAMES: &[&str] = &[
    "execute",
    "delete",
    "remove",
    "rm",
    "trash",
    "purge",
    "empty_trash",
    "cleanup_now",
    "apply_plan",
    "approve_plan",
    "shell",
    "run_command",
    "chmod",
    "chown",
    "sudo",
    "move_file",
    "write_file",
];

fn spawn_server() -> (
    Child,
    tokio::process::ChildStdin,
    BufReader<tokio::process::ChildStdout>,
) {
    let bin_path = env!("CARGO_BIN_EXE_vacua-mcp");
    let mut child = Command::new(bin_path)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn vacua-mcp binary");

    let stdin = child.stdin.take().expect("Child stdin unavailable");
    let stdout = child.stdout.take().expect("Child stdout unavailable");
    let reader = BufReader::new(stdout);

    (child, stdin, reader)
}

async fn send_request(
    writer: &mut tokio::process::ChildStdin,
    reader: &mut BufReader<tokio::process::ChildStdout>,
    id: u64,
    method: &str,
    params: Value,
) -> Value {
    let req = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let s = serde_json::to_string(&req).unwrap();
    writer.write_all(s.as_bytes()).await.unwrap();
    writer.write_all(b"\n").await.unwrap();
    writer.flush().await.unwrap();

    let mut line = String::new();
    let read_fut = reader.read_line(&mut line);
    tokio::time::timeout(Duration::from_secs(15), read_fut)
        .await
        .expect("Timed out reading MCP response")
        .expect("Failed to read line from MCP stdout");

    serde_json::from_str(&line).expect("Response is not valid JSON-RPC")
}

async fn send_notification(writer: &mut tokio::process::ChildStdin, method: &str) {
    let notif = json!({
        "jsonrpc": "2.0",
        "method": method,
    });
    let s = serde_json::to_string(&notif).unwrap();
    writer.write_all(s.as_bytes()).await.unwrap();
    writer.write_all(b"\n").await.unwrap();
    writer.flush().await.unwrap();
}

#[tokio::test]
async fn test_mcp_full_stdio_handshake_and_tools() {
    let (mut child, mut stdin, mut stdout) = spawn_server();

    // 1. Initialize Handshake
    let init_resp = send_request(
        &mut stdin,
        &mut stdout,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "vacua-integration-test",
                "version": "1.0.0"
            }
        }),
    )
    .await;

    assert_eq!(init_resp["id"], 1);
    assert!(init_resp["result"]["capabilities"]["tools"].is_object());
    assert!(init_resp["result"]["capabilities"]["resources"].is_object());
    assert!(init_resp["result"]["capabilities"]["prompts"].is_object());

    // Send initialized notification
    send_notification(&mut stdin, "notifications/initialized").await;

    // 2. tools/list
    let tools_resp = send_request(&mut stdin, &mut stdout, 2, "tools/list", json!({})).await;
    let tools = tools_resp["result"]["tools"]
        .as_array()
        .expect("tools array");
    assert!(
        tools.len() >= 12,
        "Expected at least 12 tools, got {}",
        tools.len()
    );

    let mut tool_names = BTreeSet::new();
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        tool_names.insert(name.to_string());

        // Assert no destructive words in tool name
        for destructive in DESTRUCTIVE_NAMES {
            assert!(
                !name.to_lowercase().contains(destructive),
                "Destructive tool name found: '{}' contains '{}'!",
                name,
                destructive
            );
        }

        // Assert schema exists and annotations exist
        assert!(tool["inputSchema"].is_object());
        assert!(tool["annotations"].is_object());
        assert_eq!(tool["annotations"]["destructiveHint"], false);
    }

    // Assert key tools are present
    assert!(tool_names.contains("vacua_get_capabilities"));
    assert!(tool_names.contains("vacua_storage_summary"));
    assert!(tool_names.contains("vacua_list_snapshots"));
    assert!(tool_names.contains("vacua_diff_snapshots"));
    assert!(tool_names.contains("vacua_list_candidates"));
    assert!(tool_names.contains("vacua_explain_candidate"));
    assert!(tool_names.contains("vacua_list_applications"));
    assert!(tool_names.contains("vacua_get_application"));
    assert!(tool_names.contains("vacua_list_duplicates"));
    assert!(tool_names.contains("vacua_get_duplicate_group"));
    assert!(tool_names.contains("vacua_simulate_cleanup"));
    assert!(tool_names.contains("vacua_propose_cleanup_plan"));
    assert!(tool_names.contains("vacua_history_summary"));
    assert!(tool_names.contains("vacua_verify_history"));

    // 3. resources/list
    let res_resp = send_request(&mut stdin, &mut stdout, 3, "resources/list", json!({})).await;
    let resources = res_resp["result"]["resources"]
        .as_array()
        .expect("resources array");
    assert!(resources.iter().any(|r| r["uri"] == "vacua://capabilities"));
    assert!(resources
        .iter()
        .any(|r| r["uri"] == "vacua://storage/summary"));

    // 4. resources/read
    let read_resp = send_request(
        &mut stdin,
        &mut stdout,
        4,
        "resources/read",
        json!({"uri": "vacua://capabilities"}),
    )
    .await;
    assert_eq!(read_resp["id"], 4);
    let contents = read_resp["result"]["contents"]
        .as_array()
        .expect("contents array");
    assert_eq!(contents[0]["uri"], "vacua://capabilities");
    let text = contents[0]["text"].as_str().unwrap();
    assert!(text.contains("vacua.mcp.server-capabilities.v1"));
    assert!(text.contains("\"mutation_authority\": false"));
    assert!(text.contains("\"executor_linked\": false"));

    // 5. prompts/list
    let prompts_resp = send_request(&mut stdin, &mut stdout, 5, "prompts/list", json!({})).await;
    let prompts = prompts_resp["result"]["prompts"]
        .as_array()
        .expect("prompts array");
    assert!(prompts.iter().any(|p| p["name"] == "review_storage_growth"));
    assert!(prompts
        .iter()
        .any(|p| p["name"] == "review_cleanup_proposal"));

    // 6. prompts/get
    let prompt_resp = send_request(
        &mut stdin,
        &mut stdout,
        6,
        "prompts/get",
        json!({"name": "review_cleanup_proposal"}),
    )
    .await;
    let messages = prompt_resp["result"]["messages"]
        .as_array()
        .expect("messages");
    let prompt_text = messages[0]["content"]["text"].as_str().unwrap();
    assert!(prompt_text.contains("MCP CANNOT execute this plan"));

    // 7. tools/call - vacua_get_capabilities
    let cap_resp = send_request(
        &mut stdin,
        &mut stdout,
        7,
        "tools/call",
        json!({
            "name": "vacua_get_capabilities",
            "arguments": {}
        }),
    )
    .await;
    assert_eq!(cap_resp["result"]["isError"], false);
    let structured = &cap_resp["result"]["structuredContent"];
    assert_eq!(structured["mutation_authority"], false);
    assert_eq!(structured["executor_linked"], false);
    assert_eq!(structured["read_only_tier"], true);
    assert_eq!(structured["analyze_only_tier"], true);
    assert_eq!(structured["propose_only_tier"], true);

    // 8. tools/call - vacua_storage_summary
    let storage_resp = send_request(
        &mut stdin,
        &mut stdout,
        8,
        "tools/call",
        json!({
            "name": "vacua_storage_summary",
            "arguments": {}
        }),
    )
    .await;
    assert_eq!(storage_resp["result"]["isError"], false);
    let storage = &storage_resp["result"]["structuredContent"];
    assert_eq!(storage["schema_version"], "vacua.mcp.storage-summary.v1");
    assert_eq!(storage["filesystem_type"], "apfs");
    assert!(storage["total_space_bytes"].as_u64().unwrap() > 0);

    // 9. tools/call - invalid tool returns error
    let invalid_resp = send_request(
        &mut stdin,
        &mut stdout,
        9,
        "tools/call",
        json!({
            "name": "vacua_execute_cleanup",
            "arguments": {}
        }),
    )
    .await;
    assert!(
        invalid_resp["error"].is_object(),
        "Expected error for non-existent tool"
    );

    drop(stdin);
    let _ = child.kill().await;
}
