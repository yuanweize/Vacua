use std::fs;
use std::path::PathBuf;
use vacua_api::generate_all_schemas;

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("schemas")
        .join("mcp");

    fs::create_dir_all(&out_dir).expect("Failed to create schemas/mcp directory");

    let schemas = generate_all_schemas();
    for (filename, schema) in schemas {
        let file_path = out_dir.join(filename);
        let content = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
        fs::write(&file_path, content + "\n").expect("Failed to write schema file");
        println!("Generated {}", file_path.display());
    }
}
