use std::fs;
use std::path::PathBuf;
use vacua_api::generate_all_schemas;

#[test]
fn test_no_schema_drift() {
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("schemas")
        .join("mcp");

    let schemas = generate_all_schemas();
    for (filename, expected_value) in schemas {
        let file_path = schema_dir.join(filename);
        assert!(
            file_path.exists(),
            "Schema file {} does not exist! Run cargo run -p vacua-api --bin generate_schemas",
            file_path.display()
        );

        let content = fs::read_to_string(&file_path).expect("Failed to read schema file");
        let disk_value: serde_json::Value =
            serde_json::from_str(&content).expect("Invalid JSON in schema file");

        assert_eq!(
            disk_value, expected_value,
            "Schema drift detected in {}! Run cargo run -p vacua-api --bin generate_schemas",
            filename
        );
    }
}
