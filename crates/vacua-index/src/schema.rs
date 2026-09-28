use rusqlite::Connection;
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Unsupported future schema version: database is version {found}, but this binary only supports up to version {supported}. Refusing to operate on database.")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },
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

    if current_version == 0 {
        // Initial schema migration
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

        tx.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
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
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('volumes', 'scan_sessions', 'entries', 'classifications')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables_count, 4);
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
