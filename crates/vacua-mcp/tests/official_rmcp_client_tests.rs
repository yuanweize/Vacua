use rmcp::model::*;
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;

#[tokio::test]
async fn test_official_rmcp_client_integration() -> Result<(), Box<dyn std::error::Error>> {
    let bin_path = env!("CARGO_BIN_EXE_vacua-mcp");

    // Spawn server via official rmcp TokioChildProcess transport
    let mut cmd = tokio::process::Command::new(bin_path);
    cmd.arg("--stdio");
    let transport = TokioChildProcess::new(cmd)?;

    // Perform official SDK handshake / protocol negotiation
    let client = ().serve(transport).await?;

    // 1. Tool discovery
    let tools = client.list_all_tools().await?;
    assert!(
        tools.len() >= 14,
        "Expected at least 14 tools, found {}",
        tools.len()
    );

    let mut tool_map = std::collections::HashMap::new();
    for tool in &tools {
        tool_map.insert(tool.name.to_string(), tool);
        let ann = tool.annotations.as_ref().expect("Annotations required");
        assert_eq!(
            ann.destructive_hint,
            Some(false),
            "Tool '{}' must never be destructive",
            tool.name
        );
        assert_eq!(
            ann.open_world_hint,
            Some(false),
            "Local closed-domain tool '{}' must have open_world_hint = false",
            tool.name
        );
    }

    // Duplicate tools must truthfully declare cache side-effect (read_only_hint = false)
    let dup_tool = tool_map
        .get("vacua_list_duplicates")
        .expect("vacua_list_duplicates tool");
    let dup_ann = dup_tool.annotations.as_ref().unwrap();
    assert_eq!(
        dup_ann.read_only_hint,
        Some(false),
        "Duplicate scanning mutates fingerprint cache, so read_only_hint must be false"
    );

    // 2. Resource discovery
    let resources = client.list_all_resources().await?;
    assert!(resources.iter().any(|r| r.uri == "vacua://capabilities"));
    assert!(resources.iter().any(|r| r.uri == "vacua://storage/summary"));

    // 3. Resource templates discovery
    let templates = client.list_all_resource_templates().await?;
    assert!(templates
        .iter()
        .any(|t| t.uri_template == "vacua://candidate/{candidate_id}"));
    assert!(templates
        .iter()
        .any(|t| t.uri_template == "vacua://snapshot/{snapshot_id}"));
    assert!(templates
        .iter()
        .any(|t| t.uri_template == "vacua://duplicate/{group_id}"));
    assert!(templates
        .iter()
        .any(|t| t.uri_template == "vacua://application/{application_id}"));

    // 4. Prompt discovery
    let prompts = client.list_all_prompts().await?;
    assert!(prompts.iter().any(|p| p.name == "review_storage_growth"));
    assert!(prompts.iter().any(|p| p.name == "review_cleanup_proposal"));

    // 5. Tool call: vacua_get_capabilities
    let cap_call = client
        .call_tool(CallToolRequestParams::new("vacua_get_capabilities"))
        .await?;
    assert_eq!(cap_call.is_error, Some(false));
    let cap_struct = cap_call
        .structured_content
        .as_ref()
        .expect("structured_content");
    assert_eq!(cap_struct["mutation_authority"], false);
    assert_eq!(cap_struct["executor_linked"], false);
    assert_eq!(cap_struct["plan_export_enabled"], false);

    // 6. Tool call: vacua_storage_summary
    let storage_call = client
        .call_tool(CallToolRequestParams::new("vacua_storage_summary"))
        .await?;
    assert_eq!(storage_call.is_error, Some(false));
    let storage_struct = storage_call
        .structured_content
        .as_ref()
        .expect("structured_content");
    assert_eq!(storage_struct["filesystem_type"], "apfs");

    // 7. Tool call: vacua_list_candidates with invalid cursor must fail truthfully
    let invalid_cursor_res = client
        .call_tool(
            CallToolRequestParams::new("vacua_list_candidates").with_arguments(
                serde_json::json!({
                    "cursor": "malformed_invalid_cursor_xyz"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await;
    assert!(
        invalid_cursor_res.is_err(),
        "Invalid cursor must return protocol error, never silent first page!"
    );

    // 8. Read resource: vacua://capabilities
    let read_res = client
        .read_resource(ReadResourceRequestParams::new("vacua://capabilities"))
        .await?;
    match &read_res.contents[0] {
        ResourceContents::TextResourceContents { uri, text, .. } => {
            assert_eq!(uri, "vacua://capabilities");
            assert!(text.contains("vacua.mcp.server-capabilities.v1"));
        }
        _ => panic!("Expected TextResourceContents"),
    }

    // 9. Read resource: vacua://snapshot/non-existent-snap must fail with resource not found
    let snap_res = client
        .read_resource(ReadResourceRequestParams::new(
            "vacua://snapshot/non-existent-snap",
        ))
        .await;
    assert!(
        snap_res.is_err(),
        "Non-existent snapshot resource must return error"
    );

    // Clean cancellation
    client.cancel().await?;
    Ok(())
}
