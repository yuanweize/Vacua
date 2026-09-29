use std::collections::HashMap;
use std::path::PathBuf;
use vacua_index::{IndexDatabase, StorageSnapshot};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: inspector_fixture <root_dir> <db_path>");
        std::process::exit(1);
    }
    let root_dir = PathBuf::from(&args[1])
        .canonicalize()
        .expect("canonicalize root");
    let db_path = PathBuf::from(&args[2]);

    let mut db = IndexDatabase::open(&db_path).expect("open index db");
    let snap = StorageSnapshot {
        snapshot_id: "snap-fixture-12345".to_string(),
        name: "fixture_snapshot".to_string(),
        root_path: root_dir,
        timestamp: chrono::Utc::now().timestamp(),
        total_files: 2,
        total_dirs: 1,
        logical_bytes: 4096,
        allocated_bytes: 4096,
        subtrees: HashMap::new(),
    };
    db.save_snapshot(&snap).expect("save snapshot");
    println!("{}", snap.snapshot_id);
}
