use rusqlite::Connection;
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 5;

#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Unsupported future schema version: database is version {found}, but this binary only supports up to version {supported}. Refusing to operate on database.")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },

    #[error("Database schema version {found} is older than supported version {supported}. Run normal Vacua CLI once to upgrade index.")]
    StaleSchemaVersion { found: u32, supported: u32 },
}

pub fn run_migrations(conn: &mut Connection) -> std::result::Result<(), SchemaError> {
    // Performance tuning pragmas must be set outside of any transaction
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA temp_store = MEMORY;
        "#,
    )?;

    let current_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if current_version > CURRENT_SCHEMA_VERSION {
        return Err(SchemaError::UnsupportedSchemaVersion {
            found: current_version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    if current_version < 1 {
        let tx = conn.transaction()?;

        tx.execute_batch(
            r#"
            -- Discovered filesystem volumes
            CREATE TABLE IF NOT EXISTS volumes (
                volume_uuid TEXT PRIMARY KEY,
                device_id INTEGER NOT NULL,
                mount_point TEXT NOT NULL,
                fs_type TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );

            -- Historical scan sessions audit
            CREATE TABLE IF NOT EXISTS scan_sessions (
                session_id TEXT PRIMARY KEY,
                target_path TEXT NOT NULL,
                total_files INTEGER NOT NULL,
                total_dirs INTEGER NOT NULL,
                logical_bytes INTEGER NOT NULL,
                allocated_bytes INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );

            -- Filesystem node metadata index
            CREATE TABLE IF NOT EXISTS entries (
                device_id INTEGER NOT NULL,
                inode INTEGER NOT NULL,
                canonical_path TEXT NOT NULL,
                parent_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                logical_bytes INTEGER NOT NULL,
                allocated_bytes INTEGER NOT NULL,
                mtime_sec INTEGER NOT NULL,
                observed_at INTEGER NOT NULL,
                PRIMARY KEY (device_id, inode, canonical_path)
            );
            CREATE INDEX IF NOT EXISTS idx_entries_path ON entries(canonical_path);
            CREATE INDEX IF NOT EXISTS idx_entries_parent ON entries(parent_path);

            -- Semantic candidate classifications
            CREATE TABLE IF NOT EXISTS classifications (
                candidate_id TEXT PRIMARY KEY,
                canonical_path TEXT NOT NULL,
                category TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                confidence_score REAL NOT NULL,
                reconstructable INTEGER NOT NULL,
                allocated_bytes INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_class_risk ON classifications(risk_level);
            CREATE INDEX IF NOT EXISTS idx_class_path ON classifications(canonical_path);
            "#,
        )?;

        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    }

    if current_version < 2 {
        let tx = conn.transaction()?;

        tx.execute_batch(
            r#"
            -- Persistent FSEvents watched roots & event cursors
            CREATE TABLE IF NOT EXISTS watched_roots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                watched_root TEXT UNIQUE NOT NULL,
                volume_id INTEGER NOT NULL,
                last_event_id INTEGER NOT NULL,
                last_full_scan INTEGER NOT NULL,
                status TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_watched_root ON watched_roots(watched_root);

            -- Storage state snapshots
            CREATE TABLE IF NOT EXISTS snapshots (
                snapshot_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                root_path TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                total_files INTEGER NOT NULL,
                total_dirs INTEGER NOT NULL,
                logical_bytes INTEGER NOT NULL,
                allocated_bytes INTEGER NOT NULL,
                snapshot_data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_name ON snapshots(name);
            CREATE INDEX IF NOT EXISTS idx_snapshots_time ON snapshots(timestamp DESC);
            "#,
        )?;

        tx.pragma_update(None, "user_version", 2)?;
        tx.commit()?;
    }

    if current_version < 3 {
        let tx = conn.transaction()?;

        tx.execute_batch(
            r#"
            -- Persistent content fingerprints with nanosecond resolution
            CREATE TABLE IF NOT EXISTS content_fingerprints (
                device_id INTEGER NOT NULL,
                inode INTEGER NOT NULL,
                canonical_path TEXT NOT NULL,
                logical_size INTEGER NOT NULL,
                mtime_sec INTEGER NOT NULL,
                mtime_nsec INTEGER NOT NULL,
                ctime_sec INTEGER NOT NULL,
                ctime_nsec INTEGER NOT NULL,
                sample_hash TEXT,
                full_hash TEXT,
                hash_algorithm TEXT NOT NULL,
                fingerprint_version TEXT DEFAULT '',
                observed_at INTEGER NOT NULL,
                PRIMARY KEY (device_id, inode, canonical_path)
            );
            CREATE INDEX IF NOT EXISTS idx_fingerprints_identity ON content_fingerprints(device_id, inode, logical_size, mtime_sec, mtime_nsec, ctime_sec, ctime_nsec);
            CREATE INDEX IF NOT EXISTS idx_fingerprints_full_hash ON content_fingerprints(full_hash);
            CREATE INDEX IF NOT EXISTS idx_fingerprints_path ON content_fingerprints(canonical_path);
            "#,
        )?;

        tx.pragma_update(None, "user_version", 3)?;
        tx.commit()?;
    }

    if current_version < 4 {
        let tx = conn.transaction()?;

        tx.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS content_fingerprints (
                device_id INTEGER NOT NULL,
                inode INTEGER NOT NULL,
                canonical_path TEXT NOT NULL,
                logical_size INTEGER NOT NULL,
                mtime_sec INTEGER NOT NULL,
                mtime_nsec INTEGER NOT NULL,
                ctime_sec INTEGER NOT NULL,
                ctime_nsec INTEGER NOT NULL,
                sample_hash TEXT,
                sample_version TEXT,
                full_hash TEXT,
                full_version TEXT,
                hash_algorithm TEXT NOT NULL,
                fingerprint_version TEXT DEFAULT '',
                observed_at INTEGER NOT NULL,
                PRIMARY KEY (device_id, inode, canonical_path)
            );
            CREATE INDEX IF NOT EXISTS idx_fingerprints_identity ON content_fingerprints(device_id, inode, logical_size, mtime_sec, mtime_nsec, ctime_sec, ctime_nsec);
            CREATE INDEX IF NOT EXISTS idx_fingerprints_full_hash ON content_fingerprints(full_hash);
            CREATE INDEX IF NOT EXISTS idx_fingerprints_path ON content_fingerprints(canonical_path);
            "#,
        )?;

        let has_sample_version: bool = tx
            .prepare("SELECT sample_version FROM content_fingerprints LIMIT 0")
            .is_ok();
        if !has_sample_version {
            tx.execute(
                "ALTER TABLE content_fingerprints ADD COLUMN sample_version TEXT",
                [],
            )?;
            tx.execute(
                "ALTER TABLE content_fingerprints ADD COLUMN full_version TEXT",
                [],
            )?;
            let _ = tx.execute(
                "UPDATE content_fingerprints SET sample_version = fingerprint_version WHERE sample_hash IS NOT NULL",
                [],
            );
            let _ = tx.execute(
                "UPDATE content_fingerprints SET full_version = fingerprint_version WHERE full_hash IS NOT NULL",
                [],
            );
        }

        tx.pragma_update(None, "user_version", 4)?;
        tx.commit()?;
    }

    if current_version < 5 {
        let tx = conn.transaction()?;

        tx.execute_batch(
            r#"
            -- Storage tree generations
            CREATE TABLE IF NOT EXISTS storage_tree_generations (
                generation_id TEXT PRIMARY KEY,
                root_path TEXT NOT NULL,
                root_id TEXT NOT NULL,
                observed_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                source TEXT NOT NULL,
                total_files INTEGER NOT NULL,
                total_dirs INTEGER NOT NULL,
                total_logical_bytes INTEGER NOT NULL,
                total_allocated_bytes INTEGER NOT NULL,
                coverage_json TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tree_gen_root ON storage_tree_generations(root_id, observed_at DESC);

            -- Storage tree nodes
            CREATE TABLE IF NOT EXISTS storage_tree_nodes (
                generation_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                parent_node_id TEXT,
                raw_relative_path BLOB NOT NULL,
                display_name TEXT NOT NULL,
                display_path TEXT NOT NULL,
                kind TEXT NOT NULL,
                depth INTEGER NOT NULL,
                direct_logical_bytes INTEGER NOT NULL,
                direct_allocated_bytes INTEGER NOT NULL,
                subtree_logical_bytes INTEGER NOT NULL,
                subtree_allocated_bytes INTEGER NOT NULL,
                file_count INTEGER NOT NULL,
                directory_count INTEGER NOT NULL,
                hardlink_alias_count INTEGER NOT NULL,
                is_hardlink_alias INTEGER NOT NULL DEFAULT 0,
                child_count INTEGER NOT NULL,
                mtime_sec INTEGER NOT NULL,
                PRIMARY KEY (generation_id, node_id)
            );
            CREATE INDEX IF NOT EXISTS idx_tree_nodes_parent_alloc ON storage_tree_nodes(generation_id, parent_node_id, subtree_allocated_bytes DESC);
            CREATE INDEX IF NOT EXISTS idx_tree_nodes_parent_logic ON storage_tree_nodes(generation_id, parent_node_id, subtree_logical_bytes DESC);
            CREATE INDEX IF NOT EXISTS idx_tree_nodes_gen_node ON storage_tree_nodes(generation_id, node_id);
            "#,
        )?;

        tx.pragma_update(None, "user_version", 5)?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_create_tables_and_set_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();

        let version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // Verify table existence
        let tables_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('volumes', 'scan_sessions', 'entries', 'classifications', 'watched_roots', 'snapshots', 'content_fingerprints', 'storage_tree_generations', 'storage_tree_nodes')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables_count, 9);
    }

    #[test]
    fn test_migrations_v2_to_v3_preserves_data() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Run migrations up to v2 manually
        conn.execute_batch(
            r#"
            CREATE TABLE watched_roots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                watched_root TEXT UNIQUE NOT NULL,
                volume_id INTEGER NOT NULL,
                last_event_id INTEGER NOT NULL,
                last_full_scan INTEGER NOT NULL,
                status TEXT NOT NULL
            );
            CREATE TABLE snapshots (
                snapshot_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                root_path TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                total_files INTEGER NOT NULL,
                total_dirs INTEGER NOT NULL,
                logical_bytes INTEGER NOT NULL,
                allocated_bytes INTEGER NOT NULL,
                snapshot_data TEXT NOT NULL
            );
            INSERT INTO snapshots VALUES ('snap-1', 'test', '/tmp', 1234, 10, 2, 1000, 2000, '{}');
            PRAGMA user_version = 2;
            "#,
        )
        .unwrap();

        // Now run full migrations to upgrade to v4
        run_migrations(&mut conn).unwrap();

        let version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // Check preserved snapshot data
        let snap_name: String = conn
            .query_row(
                "SELECT name FROM snapshots WHERE snapshot_id = 'snap-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(snap_name, "test");

        // Check new table exists
        let fp_exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name = 'content_fingerprints'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fp_exists, 1);
    }

    #[test]
    fn test_migrations_v3_to_v4_adds_columns_and_migrates_data() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Run migrations up to v3 manually
        conn.execute_batch(
            r#"
            CREATE TABLE content_fingerprints (
                device_id INTEGER NOT NULL,
                inode INTEGER NOT NULL,
                canonical_path TEXT NOT NULL,
                logical_size INTEGER NOT NULL,
                mtime_sec INTEGER NOT NULL,
                mtime_nsec INTEGER NOT NULL,
                ctime_sec INTEGER NOT NULL,
                ctime_nsec INTEGER NOT NULL,
                sample_hash TEXT,
                full_hash TEXT,
                hash_algorithm TEXT NOT NULL,
                fingerprint_version TEXT NOT NULL,
                observed_at INTEGER NOT NULL,
                PRIMARY KEY (device_id, inode, canonical_path)
            );
            INSERT INTO content_fingerprints VALUES (
                1, 100, '/tmp/test.txt', 1024, 100, 200, 300, 400,
                'sample123', 'full456', 'BLAKE3', 'legacy-v1', 12345
            );
            PRAGMA user_version = 3;
            "#,
        )
        .unwrap();

        // Run migrations to upgrade to v4
        run_migrations(&mut conn).unwrap();

        let version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // Verify columns were added and legacy data populated
        let (s_ver, f_ver): (String, String) = conn
            .query_row(
                "SELECT sample_version, full_version FROM content_fingerprints WHERE inode = 100",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(s_ver, "legacy-v1");
        assert_eq!(f_ver, "legacy-v1");
    }

    #[test]
    fn test_migrations_v4_to_v5_preserves_data() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Run migrations up to v4 manually
        conn.execute_batch(
            r#"
            CREATE TABLE entries (
                device_id INTEGER NOT NULL,
                inode INTEGER NOT NULL,
                canonical_path TEXT NOT NULL,
                parent_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                logical_bytes INTEGER NOT NULL,
                allocated_bytes INTEGER NOT NULL,
                mtime_sec INTEGER NOT NULL,
                observed_at INTEGER NOT NULL,
                PRIMARY KEY (device_id, inode, canonical_path)
            );
            INSERT INTO entries VALUES (1, 100, '/tmp/a.txt', '/tmp', 'file', 1024, 4096, 1000, 2000);
            PRAGMA user_version = 4;
            "#,
        )
        .unwrap();

        // Run migrations to upgrade to v5
        run_migrations(&mut conn).unwrap();

        let version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // Verify existing entry preserved
        let path: String = conn
            .query_row(
                "SELECT canonical_path FROM entries WHERE inode = 100",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(path, "/tmp/a.txt");

        // Verify new tables created
        let gen_table_exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('storage_tree_generations', 'storage_tree_nodes')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(gen_table_exists, 2);
    }

    #[test]
    fn test_migrations_refuse_future_schema_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();

        let err = run_migrations(&mut conn).unwrap_err();
        match err {
            SchemaError::UnsupportedSchemaVersion { found, supported } => {
                assert_eq!(found, 999);
                assert_eq!(supported, CURRENT_SCHEMA_VERSION);
            }
            _ => panic!("Expected UnsupportedSchemaVersion error"),
        }
    }
}
