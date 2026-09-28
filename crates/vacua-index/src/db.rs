use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

use vacua_core::candidate::Candidate;
use vacua_scan::entry::ScannedEntry;
use vacua_scan::ScanReport;

use crate::schema::{run_migrations, SchemaError};

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("Database schema error: {0}")]
    Schema(#[from] SchemaError),

    #[error("SQLite query error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedEntry {
    pub device_id: u64,
    pub inode: u64,
    pub canonical_path: PathBuf,
    pub parent_path: PathBuf,
    pub file_type: String,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub mtime_sec: i64,
    pub observed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_entries: u64,
    pub total_logical_bytes: u64,
    pub total_allocated_bytes: u64,
    pub total_sessions: u64,
    pub last_scan_timestamp: Option<i64>,
}

pub struct IndexDatabase {
    conn: Connection,
}

impl IndexDatabase {
    pub fn open(path: &Path) -> Result<Self, IndexError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path)?;
        run_migrations(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self, IndexError> {
        let mut conn = Connection::open_in_memory()?;
        run_migrations(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn record_session(
        &mut self,
        target_path: &Path,
        report: &ScanReport,
    ) -> Result<String, IndexError> {
        let session_id = format!("scan-{}", Utc::now().timestamp_millis());
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO scan_sessions (
                session_id, target_path, total_files, total_dirs,
                logical_bytes, allocated_bytes, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                session_id,
                target_path.to_string_lossy(),
                report.total_files,
                report.total_dirs,
                report.allocation.logical_bytes,
                report.allocation.allocated_bytes,
                now,
            ],
        )?;

        Ok(session_id)
    }

    pub fn upsert_entries(&mut self, entries: &[ScannedEntry]) -> Result<usize, IndexError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;

        let mut stmt = tx.prepare_cached(
            r#"
            INSERT INTO entries (
                device_id, inode, canonical_path, parent_path,
                file_type, logical_bytes, allocated_bytes, mtime_sec, observed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(device_id, inode, canonical_path) DO UPDATE SET
                logical_bytes = excluded.logical_bytes,
                allocated_bytes = excluded.allocated_bytes,
                mtime_sec = excluded.mtime_sec,
                observed_at = excluded.observed_at
            "#,
        )?;

        let mut count = 0;
        for e in entries {
            let path_str = e.path.to_string_lossy();
            let parent_str = e
                .path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let file_type = if e.is_symlink {
                "symlink"
            } else if e.is_dir {
                "directory"
            } else {
                "file"
            };

            stmt.execute(params![
                e.device_id,
                e.inode,
                path_str,
                parent_str,
                file_type,
                e.logical_bytes,
                e.allocated_bytes,
                e.mtime_sec,
                now,
            ])?;
            count += 1;
        }

        drop(stmt);
        tx.commit()?;
        Ok(count)
    }

    pub fn record_classifications(
        &mut self,
        candidates: &[Candidate],
    ) -> Result<usize, IndexError> {
        let now = Utc::now().timestamp();
        let tx = self.conn.transaction()?;

        let mut stmt = tx.prepare_cached(
            r#"
            INSERT INTO classifications (
                candidate_id, canonical_path, category, risk_level,
                confidence_score, reconstructable, allocated_bytes, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(candidate_id) DO UPDATE SET
                category = excluded.category,
                risk_level = excluded.risk_level,
                confidence_score = excluded.confidence_score,
                reconstructable = excluded.reconstructable,
                allocated_bytes = excluded.allocated_bytes,
                updated_at = excluded.updated_at
            "#,
        )?;

        let mut count = 0;
        for c in candidates {
            stmt.execute(params![
                c.id,
                c.path.to_string_lossy(),
                c.category.to_string(),
                c.risk.to_string(),
                c.confidence_score,
                c.reconstructable as i32,
                c.allocation.allocated_bytes,
                now,
            ])?;
            count += 1;
        }

        drop(stmt);
        tx.commit()?;
        Ok(count)
    }

    pub fn get_entry_by_path(&self, path: &Path) -> Result<Option<IndexedEntry>, IndexError> {
        let path_str = path.to_string_lossy();
        let mut stmt = self.conn.prepare(
            r#"
            SELECT device_id, inode, canonical_path, parent_path,
                   file_type, logical_bytes, allocated_bytes, mtime_sec, observed_at
            FROM entries WHERE canonical_path = ?1 LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query(params![path_str])?;
        if let Some(row) = rows.next()? {
            let path_s: String = row.get(2)?;
            let parent_s: String = row.get(3)?;
            Ok(Some(IndexedEntry {
                device_id: row.get(0)?,
                inode: row.get(1)?,
                canonical_path: PathBuf::from(path_s),
                parent_path: PathBuf::from(parent_s),
                file_type: row.get(4)?,
                logical_bytes: row.get(5)?,
                allocated_bytes: row.get(6)?,
                mtime_sec: row.get(7)?,
                observed_at: row.get(8)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_index_stats(&self) -> Result<IndexStats, IndexError> {
        let (total_entries, total_logical, total_allocated): (u64, u64, u64) = self
            .conn
            .query_row(
                "SELECT count(*), coalesce(sum(logical_bytes), 0), coalesce(sum(allocated_bytes), 0) FROM entries",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;

        let (total_sessions, last_ts): (u64, Option<i64>) = self.conn.query_row(
            "SELECT count(*), max(created_at) FROM scan_sessions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        Ok(IndexStats {
            total_entries,
            total_logical_bytes: total_logical,
            total_allocated_bytes: total_allocated,
            total_sessions,
            last_scan_timestamp: last_ts,
        })
    }

    pub fn rebuild(&mut self) -> Result<(), IndexError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM classifications", [])?;
        tx.execute("DELETE FROM entries", [])?;
        tx.execute("DELETE FROM scan_sessions", [])?;
        tx.commit()?;
        self.conn.execute("VACUUM", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_upsert_and_retrieve_stats() {
        let mut db = IndexDatabase::open_in_memory().unwrap();

        let scanned = ScannedEntry {
            path: PathBuf::from("/test/file.bin"),
            logical_bytes: 5000,
            allocated_bytes: 8192,
            inode: 101,
            device_id: 1,
            is_dir: false,
            is_symlink: false,
            is_sparse: false,
            nlink: 1,
            mtime_sec: 1000,
            ctime_sec: 1000,
        };

        let inserted = db.upsert_entries(&[scanned]).unwrap();
        assert_eq!(inserted, 1);

        let retrieved = db
            .get_entry_by_path(Path::new("/test/file.bin"))
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.logical_bytes, 5000);
        assert_eq!(retrieved.allocated_bytes, 8192);
        assert_eq!(retrieved.inode, 101);

        let stats = db.get_index_stats().unwrap();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.total_logical_bytes, 5000);
        assert_eq!(stats.total_allocated_bytes, 8192);

        // Test rebuild clears entries
        db.rebuild().unwrap();
        let stats_after = db.get_index_stats().unwrap();
        assert_eq!(stats_after.total_entries, 0);
    }
}
