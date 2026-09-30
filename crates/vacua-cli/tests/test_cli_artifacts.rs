use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;
use vacua_artifacts::{ArtifactPersistence, DeveloperArtifactScanner};
use vacua_index::IndexDatabase;

#[test]
fn test_cli_artifacts_end_to_end_flow() {
    let sandbox = tempdir().unwrap();
    let root = sandbox.path();

    // 1. Setup mock Cargo project
    let cargo_proj = root.join("my-rust-crate");
    fs::create_dir_all(cargo_proj.join("src")).unwrap();
    fs::create_dir_all(cargo_proj.join("target/debug")).unwrap();
    fs::write(
        cargo_proj.join("Cargo.toml"),
        b"[package]\nname = \"my-rust-crate\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(cargo_proj.join("Cargo.lock"), b"# lockfile\n").unwrap();
    fs::write(cargo_proj.join("src/main.rs"), b"fn main() {}\n").unwrap();
    let mut build_out = File::create(cargo_proj.join("target/debug/app")).unwrap();
    build_out.write_all(&vec![0xCC; 16384]).unwrap();

    // 2. Setup misleading generic build dir without manifest (should NOT be classified)
    let random_dir = root.join("Photos/target");
    fs::create_dir_all(&random_dir).unwrap();
    fs::write(random_dir.join("photo.jpg"), b"fake photo").unwrap();

    // 3. Scan with DeveloperArtifactScanner
    let root_id = "test_root_cli_123";
    let scanner = DeveloperArtifactScanner::new(root, root_id);
    let gen = scanner.scan().expect("Artifact scan should succeed");

    assert_eq!(gen.projects.len(), 1);
    let proj = &gen.projects[0];
    assert_eq!(proj.display_name, "my-rust-crate");
    assert_eq!(proj.artifacts.len(), 1);
    assert_eq!(proj.artifacts[0].display_name, "target");

    // 4. Persistence into IndexDatabase
    let mut db = IndexDatabase::open_in_memory().unwrap();
    ArtifactPersistence::publish_generation(db.conn_mut(), &gen).unwrap();

    // 5. Query latest ready generation
    let retrieved = ArtifactPersistence::get_latest_ready_generation(db.conn(), root_id)
        .unwrap()
        .expect("Ready generation should be retrieved");
    assert_eq!(retrieved.generation_id, gen.generation_id);
    assert_eq!(retrieved.projects.len(), 1);

    // 6. Query single artifact
    let art_id = &gen.projects[0].artifacts[0].artifact_id;
    let single_art = ArtifactPersistence::get_artifact(db.conn(), art_id.as_str())
        .unwrap()
        .expect("Single artifact should be found");
    assert_eq!(single_art.artifact_id, *art_id);
    assert!(single_art.rebuild_evidence.manifest_present);
    assert!(single_art.rebuild_evidence.lockfile_present);
}
