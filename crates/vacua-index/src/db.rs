use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

use vacua_core::candidate::Candidate;
use vacua_scan::entry::ScannedEntry;
use vacua_scan::{FilesystemScanner, ScanOptions, ScanReport};

use crate::fsevents::{get_current_event_id, replay_fsevents_since};
use crate::schema::{run_migrations, SchemaError};

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("Database schema error: {0}")]
    Schema(#[from] SchemaError),

    #[error("SQLite query error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(String),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedRootRecord {
    pub id: i64,
    pub watched_root: PathBuf,
    pub volume_id: u64,
    pub last_event_id: u64,
    pub last_full_scan: i64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSubtreeStats {
    #[serde(default)]
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    #[serde(default)]
    pub file_count: u64,
    #[serde(default)]
    pub dir_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SubtreeEntry {
    Detailed(SnapshotSubtreeStats),
    LegacyBytes(u64),
}

impl SubtreeEntry {
    pub fn to_stats(&self) -> SnapshotSubtreeStats {
        match self {
            SubtreeEntry::Detailed(s) => s.clone(),
            SubtreeEntry::LegacyBytes(b) => SnapshotSubtreeStats {
                logical_bytes: *b,
                allocated_bytes: *b,
                file_count: 0,
                dir_count: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSnapshot {
    pub snapshot_id: String,
    pub name: String,
    pub root_path: PathBuf,
    pub timestamp: i64,
    pub total_files: u64,
    pub total_dirs: u64,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub subtrees: HashMap<String, SnapshotSubtreeStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSnapshotSummary {
    pub snapshot_id: String,
    pub name: String,
    pub root_path: PathBuf,
    pub timestamp: i64,
    pub total_files: u64,
    pub allocated_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtreeDelta {
    pub path: String,
    pub delta_bytes: i64,
    pub allocated_delta_bytes: i64,
    pub logical_delta_bytes: i64,
    pub file_count_delta: i64,
    pub change_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub base_name: String,
    pub target_name: String,
    pub allocated_delta_bytes: i64,
    pub logical_delta_bytes: i64,
    pub files_delta: i64,
    pub subtree_deltas: Vec<SubtreeDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalRefreshResult {
    pub root_path: PathBuf,
    pub full_rescan_performed: bool,
    pub dirty_subtrees_count: usize,
    pub rescanned_subtrees: Vec<PathBuf>,
    pub updated_entries_count: usize,
    pub previous_event_id: u64,
    pub new_event_id: u64,
    pub status: String,
}

struct ExistingFingerprintRow {
    logical_size: u64,
    mtime_sec: i64,
    mtime_nsec: i64,
    ctime_sec: i64,
    ctime_nsec: i64,
    sample_hash: Option<String>,
    sample_version: Option<String>,
    full_hash: Option<String>,
    full_version: Option<String>,
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

    /// Open existing index database strictly read-only without executing migrations or creating files.
    pub fn open_read_only(path: &Path) -> Result<Self, IndexError> {
        if !path.exists() {
            return Err(IndexError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Index database not found at {}", path.display()),
            )));
        }

        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        let current_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if current_version < crate::schema::CURRENT_SCHEMA_VERSION {
            return Err(IndexError::Schema(SchemaError::StaleSchemaVersion {
                found: current_version,
                supported: crate::schema::CURRENT_SCHEMA_VERSION,
            }));
        }
        if current_version > crate::schema::CURRENT_SCHEMA_VERSION {
            return Err(IndexError::Schema(SchemaError::UnsupportedSchemaVersion {
                found: current_version,
                supported: crate::schema::CURRENT_SCHEMA_VERSION,
            }));
        }

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

    // -----------------------------------------------------------------------
    // FSEvents Persistent Cursor & Watched Roots
    // -----------------------------------------------------------------------

    pub fn get_watched_root(&self, root: &Path) -> Result<Option<WatchedRootRecord>, IndexError> {
        let path_str = root.to_string_lossy();
        let mut stmt = self.conn.prepare(
            "SELECT id, watched_root, volume_id, last_event_id, last_full_scan, status
             FROM watched_roots WHERE watched_root = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![path_str])?;
        if let Some(row) = rows.next()? {
            let r_str: String = row.get(1)?;
            Ok(Some(WatchedRootRecord {
                id: row.get(0)?,
                watched_root: PathBuf::from(r_str),
                volume_id: row.get(2)?,
                last_event_id: row.get(3)?,
                last_full_scan: row.get(4)?,
                status: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_watched_root(
        &mut self,
        root: &Path,
        volume_id: u64,
        last_event_id: u64,
        status: &str,
    ) -> Result<(), IndexError> {
        let now = Utc::now().timestamp();
        self.conn.execute(
            r#"
            INSERT INTO watched_roots (
                watched_root, volume_id, last_event_id, last_full_scan, status
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(watched_root) DO UPDATE SET
                volume_id = excluded.volume_id,
                last_event_id = excluded.last_event_id,
                last_full_scan = excluded.last_full_scan,
                status = excluded.status
            "#,
            params![
                root.to_string_lossy(),
                volume_id,
                last_event_id,
                now,
                status
            ],
        )?;
        Ok(())
    }

    pub fn list_watched_roots(&self) -> Result<Vec<WatchedRootRecord>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, watched_root, volume_id, last_event_id, last_full_scan, status
             FROM watched_roots ORDER BY last_full_scan DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let r_str: String = row.get(1)?;
            Ok(WatchedRootRecord {
                id: row.get(0)?,
                watched_root: PathBuf::from(r_str),
                volume_id: row.get(2)?,
                last_event_id: row.get(3)?,
                last_full_scan: row.get(4)?,
                status: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Surgical incremental refresh using native FSEvents replay.
    /// If no cursor exists or if dropped events require full rescan, full scan is performed.
    /// Otherwise, only dirty subtrees are rescanned and updated in SQLite!
    pub fn refresh_root(
        &mut self,
        root: &Path,
        scanner: &FilesystemScanner,
    ) -> Result<IncrementalRefreshResult, IndexError> {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let current_cur = self.get_watched_root(&canonical)?;

        let current_event_id = get_current_event_id();
        let volume_id = std::fs::symlink_metadata(&canonical)
            .map(|m| {
                use std::os::unix::fs::MetadataExt;
                m.dev()
            })
            .unwrap_or(0);

        match current_cur {
            None => {
                // Initial scan
                let report = scanner.scan(&canonical)?;
                self.record_session(&canonical, &report)?;
                let count = self.upsert_entries(&report.entries)?;
                self.upsert_watched_root(
                    &canonical,
                    volume_id,
                    current_event_id,
                    "fresh_initialized",
                )?;

                Ok(IncrementalRefreshResult {
                    root_path: canonical,
                    full_rescan_performed: true,
                    dirty_subtrees_count: 0,
                    rescanned_subtrees: vec![],
                    updated_entries_count: count,
                    previous_event_id: 0,
                    new_event_id: current_event_id,
                    status: "fresh_initialized".into(),
                })
            }
            Some(cur) => {
                let prev_event_id = cur.last_event_id;

                // 1. Volume ID check: volume changed forces full rescan
                if cur.volume_id != volume_id {
                    let report = scanner.scan(&canonical)?;
                    self.record_session(&canonical, &report)?;
                    let count = self.upsert_entries(&report.entries)?;
                    let new_id = current_event_id;
                    self.upsert_watched_root(
                        &canonical,
                        volume_id,
                        new_id,
                        "volume_changed_full_rescan",
                    )?;
                    return Ok(IncrementalRefreshResult {
                        root_path: canonical,
                        full_rescan_performed: true,
                        dirty_subtrees_count: 0,
                        rescanned_subtrees: vec![],
                        updated_entries_count: count,
                        previous_event_id: prev_event_id,
                        new_event_id: new_id,
                        status: "volume_changed_full_rescan".into(),
                    });
                }

                // 2. Replay native FSEvents since cursor
                let tracker = replay_fsevents_since(&canonical, prev_event_id)?;

                if tracker.is_full_rescan_required() {
                    // Fallback to full rescan if events dropped or wrapped
                    let report = scanner.scan(&canonical)?;
                    self.record_session(&canonical, &report)?;
                    let count = self.upsert_entries(&report.entries)?;
                    let new_id = tracker.last_event_id().max(current_event_id);
                    self.upsert_watched_root(
                        &canonical,
                        volume_id,
                        new_id,
                        "fallback_full_rescan",
                    )?;

                    Ok(IncrementalRefreshResult {
                        root_path: canonical,
                        full_rescan_performed: true,
                        dirty_subtrees_count: 0,
                        rescanned_subtrees: vec![],
                        updated_entries_count: count,
                        previous_event_id: prev_event_id,
                        new_event_id: new_id,
                        status: "fallback_full_rescan".into(),
                    })
                } else {
                    let subtrees = tracker.get_subtrees_to_rescan().unwrap_or_default();
                    if subtrees.is_empty() {
                        // Clean: no events logged
                        let new_id = tracker.last_event_id().max(prev_event_id);
                        self.upsert_watched_root(&canonical, volume_id, new_id, "clean")?;

                        Ok(IncrementalRefreshResult {
                            root_path: canonical,
                            full_rescan_performed: false,
                            dirty_subtrees_count: 0,
                            rescanned_subtrees: vec![],
                            updated_entries_count: 0,
                            previous_event_id: prev_event_id,
                            new_event_id: new_id,
                            status: "clean_up_to_date".into(),
                        })
                    } else {
                        // Surgical reconciliation of dirty subtrees
                        let total_updated = self.reconcile_subtrees(&canonical, &subtrees)?;

                        let new_id = tracker.last_event_id().max(current_event_id);
                        self.upsert_watched_root(&canonical, volume_id, new_id, "incremental")?;

                        Ok(IncrementalRefreshResult {
                            root_path: canonical,
                            full_rescan_performed: false,
                            dirty_subtrees_count: subtrees.len(),
                            rescanned_subtrees: subtrees,
                            updated_entries_count: total_updated,
                            previous_event_id: prev_event_id,
                            new_event_id: new_id,
                            status: "surgical_incremental".into(),
                        })
                    }
                }
            }
        }
    }

    /// Reconciles the SQLite index under specific subtrees against the current filesystem state.
    /// Safely cleans up deleted and renamed records while upserting newly added or modified entries.
    pub fn reconcile_subtrees(
        &mut self,
        _watched_root: &Path,
        subtrees: &[PathBuf],
    ) -> Result<usize, IndexError> {
        let mut total_updated = 0;
        for dirty_dir in subtrees {
            let dirty_str = dirty_dir.to_string_lossy().to_string();

            if !dirty_dir.exists() {
                // Subtree was deleted: delete all indexed entries under dirty_dir
                self.conn.execute(
                    "DELETE FROM entries WHERE canonical_path = ?1 OR canonical_path LIKE ?1 || '/%'",
                    params![dirty_str],
                )?;
            } else {
                // Subtree exists: reconcile added, modified, renamed, and deleted paths
                let mut existing_paths = HashSet::new();
                {
                    let mut stmt = self.conn.prepare(
                        "SELECT canonical_path FROM entries WHERE canonical_path = ?1 OR canonical_path LIKE ?1 || '/%'",
                    )?;
                    let existing_rows =
                        stmt.query_map(params![dirty_str], |r| r.get::<_, String>(0))?;
                    for p in existing_rows {
                        existing_paths.insert(p?);
                    }
                }

                let sub_scanner = FilesystemScanner::new(ScanOptions {
                    cross_mounts: false,
                    max_depth: None,
                    jobs: Some(2),
                });

                if let Ok(sub_report) = sub_scanner.scan(dirty_dir) {
                    let mut new_paths = HashSet::new();
                    for entry in &sub_report.entries {
                        new_paths.insert(entry.path.to_string_lossy().to_string());
                    }

                    // Delete stale entries that no longer exist
                    for old_p in &existing_paths {
                        if !new_paths.contains(old_p) {
                            self.conn.execute(
                                "DELETE FROM entries WHERE canonical_path = ?1",
                                params![old_p],
                            )?;
                        }
                    }

                    // Upsert current entries
                    total_updated += self.upsert_entries(&sub_report.entries)?;
                }
            }
        }
        Ok(total_updated)
    }

    // -----------------------------------------------------------------------
    // Storage Snapshots & Diff Engine
    // -----------------------------------------------------------------------

    pub fn save_snapshot(&mut self, snapshot: &StorageSnapshot) -> Result<(), IndexError> {
        let serialized = serde_json::to_string(&snapshot.subtrees)?;
        self.conn.execute(
            r#"
            INSERT INTO snapshots (
                snapshot_id, name, root_path, timestamp,
                total_files, total_dirs, logical_bytes, allocated_bytes, snapshot_data
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(snapshot_id) DO UPDATE SET
                name = excluded.name,
                snapshot_data = excluded.snapshot_data
            "#,
            params![
                snapshot.snapshot_id,
                snapshot.name,
                snapshot.root_path.to_string_lossy(),
                snapshot.timestamp,
                snapshot.total_files,
                snapshot.total_dirs,
                snapshot.logical_bytes,
                snapshot.allocated_bytes,
                serialized,
            ],
        )?;
        Ok(())
    }

    pub fn list_snapshots(&self) -> Result<Vec<StorageSnapshotSummary>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT snapshot_id, name, root_path, timestamp, total_files, allocated_bytes
             FROM snapshots ORDER BY timestamp DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let p_str: String = row.get(2)?;
            Ok(StorageSnapshotSummary {
                snapshot_id: row.get(0)?,
                name: row.get(1)?,
                root_path: PathBuf::from(p_str),
                timestamp: row.get(3)?,
                total_files: row.get(4)?,
                allocated_bytes: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_snapshot(&self, name_or_id: &str) -> Result<Option<StorageSnapshot>, IndexError> {
        let mut stmt = self.conn.prepare(
            "SELECT snapshot_id, name, root_path, timestamp, total_files, total_dirs,
                    logical_bytes, allocated_bytes, snapshot_data
             FROM snapshots WHERE name = ?1 OR snapshot_id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![name_or_id])?;
        if let Some(row) = rows.next()? {
            let root_s: String = row.get(2)?;
            let raw_data: String = row.get(8)?;
            let raw_subtrees: HashMap<String, SubtreeEntry> = serde_json::from_str(&raw_data)?;
            let subtrees = raw_subtrees
                .into_iter()
                .map(|(k, v)| (k, v.to_stats()))
                .collect();
            Ok(Some(StorageSnapshot {
                snapshot_id: row.get(0)?,
                name: row.get(1)?,
                root_path: PathBuf::from(root_s),
                timestamp: row.get(3)?,
                total_files: row.get(4)?,
                total_dirs: row.get(5)?,
                logical_bytes: row.get(6)?,
                allocated_bytes: row.get(7)?,
                subtrees,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn diff_snapshots(
        &self,
        base_name: &str,
        target_name: &str,
    ) -> Result<SnapshotDiff, IndexError> {
        let base = self
            .get_snapshot(base_name)?
            .ok_or_else(|| IndexError::SnapshotNotFound(base_name.to_string()))?;
        let target = if let Some(snap) = self.get_snapshot(target_name)? {
            snap
        } else if target_name == "current" {
            let scanner = vacua_scan::FilesystemScanner::new(vacua_scan::ScanOptions {
                cross_mounts: false,
                max_depth: None,
                ..Default::default()
            });
            let report = scanner.scan(&base.root_path)?;
            let subtrees = build_recursive_subtrees(&base.root_path, &report.entries);
            StorageSnapshot {
                snapshot_id: "current".into(),
                name: "current".into(),
                root_path: base.root_path.clone(),
                timestamp: chrono::Utc::now().timestamp(),
                total_files: report.total_files,
                total_dirs: report.total_dirs,
                logical_bytes: report.allocation.logical_bytes,
                allocated_bytes: report.allocation.allocated_bytes,
                subtrees,
            }
        } else {
            return Err(IndexError::SnapshotNotFound(target_name.to_string()));
        };

        let allocated_delta = target.allocated_bytes as i64 - base.allocated_bytes as i64;
        let logical_delta = target.logical_bytes as i64 - base.logical_bytes as i64;
        let files_delta = target.total_files as i64 - base.total_files as i64;

        let mut all_paths: HashSet<String> = HashSet::new();
        all_paths.extend(base.subtrees.keys().cloned());
        all_paths.extend(target.subtrees.keys().cloned());

        let mut subtree_deltas = Vec::new();
        for p in all_paths {
            let base_stat = base
                .subtrees
                .get(&p)
                .cloned()
                .unwrap_or(SnapshotSubtreeStats {
                    logical_bytes: 0,
                    allocated_bytes: 0,
                    file_count: 0,
                    dir_count: 0,
                });
            let target_stat = target
                .subtrees
                .get(&p)
                .cloned()
                .unwrap_or(SnapshotSubtreeStats {
                    logical_bytes: 0,
                    allocated_bytes: 0,
                    file_count: 0,
                    dir_count: 0,
                });

            let alloc_delta = target_stat.allocated_bytes as i64 - base_stat.allocated_bytes as i64;
            let log_delta = target_stat.logical_bytes as i64 - base_stat.logical_bytes as i64;
            let f_delta = target_stat.file_count as i64 - base_stat.file_count as i64;

            if alloc_delta != 0 || log_delta != 0 || f_delta != 0 {
                let change_type = if base_stat.allocated_bytes == 0 && base_stat.file_count == 0 {
                    "created".to_string()
                } else if target_stat.allocated_bytes == 0 && target_stat.file_count == 0 {
                    "removed".to_string()
                } else if alloc_delta > 0 {
                    "grown".to_string()
                } else if alloc_delta < 0 {
                    "shrunk".to_string()
                } else {
                    "unchanged".to_string()
                };

                subtree_deltas.push(SubtreeDelta {
                    path: p,
                    delta_bytes: alloc_delta,
                    allocated_delta_bytes: alloc_delta,
                    logical_delta_bytes: log_delta,
                    file_count_delta: f_delta,
                    change_type,
                });
            }
        }

        // Sort largest allocated growth first
        subtree_deltas.sort_by_key(|b| std::cmp::Reverse(b.allocated_delta_bytes.abs()));

        Ok(SnapshotDiff {
            base_name: base.name,
            target_name: target.name,
            allocated_delta_bytes: allocated_delta,
            logical_delta_bytes: logical_delta,
            files_delta,
            subtree_deltas,
        })
    }

    pub fn rebuild(&mut self) -> Result<(), IndexError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM classifications", [])?;
        tx.execute("DELETE FROM entries", [])?;
        tx.execute("DELETE FROM scan_sessions", [])?;
        tx.execute("DELETE FROM watched_roots", [])?;
        tx.execute("DELETE FROM snapshots", [])?;
        tx.commit()?;
        self.conn.execute("VACUUM", [])?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_content_fingerprint(
        &self,
        device_id: u64,
        inode: u64,
        canonical_path: &Path,
        logical_size: u64,
        mtime_sec: i64,
        mtime_nsec: i64,
        ctime_sec: i64,
        ctime_nsec: i64,
        expected_sample_version: Option<&str>,
        expected_full_version: Option<&str>,
    ) -> Result<Option<CachedFingerprint>, IndexError> {
        let path_str = canonical_path.to_string_lossy();
        let mut stmt = self.conn.prepare(
            r#"
            SELECT sample_hash, sample_version, full_hash, full_version, hash_algorithm, observed_at
            FROM content_fingerprints
            WHERE device_id = ?1 AND inode = ?2 AND canonical_path = ?3
              AND logical_size = ?4 AND mtime_sec = ?5 AND mtime_nsec = ?6
              AND ctime_sec = ?7 AND ctime_nsec = ?8
            "#,
        )?;

        let mut rows = stmt.query(rusqlite::params![
            device_id,
            inode,
            path_str.as_ref(),
            logical_size,
            mtime_sec,
            mtime_nsec,
            ctime_sec,
            ctime_nsec,
        ])?;

        if let Some(row) = rows.next()? {
            let sample_hash: Option<String> = row.get(0)?;
            let sample_version: Option<String> = row.get(1)?;
            let full_hash: Option<String> = row.get(2)?;
            let full_version: Option<String> = row.get(3)?;
            let hash_algorithm: String = row.get(4)?;
            let observed_at: i64 = row.get(5)?;

            // Only BLAKE3 is valid algorithm
            if hash_algorithm != "BLAKE3" {
                return Ok(None);
            }

            // Invalidate sample if sample_version does not match expected
            let valid_sample = if let (Some(expected), Some(actual)) =
                (expected_sample_version, sample_version.as_deref())
            {
                if expected == actual {
                    sample_hash
                } else {
                    None
                }
            } else if expected_sample_version.is_none() {
                sample_hash
            } else {
                None
            };

            // Invalidate full if full_version does not match expected
            let valid_full = if let (Some(expected), Some(actual)) =
                (expected_full_version, full_version.as_deref())
            {
                if expected == actual {
                    full_hash
                } else {
                    None
                }
            } else if expected_full_version.is_none() {
                full_hash
            } else {
                None
            };

            if valid_sample.is_none() && valid_full.is_none() {
                return Ok(None);
            }

            Ok(Some(CachedFingerprint {
                sample_hash: valid_sample,
                sample_version,
                full_hash: valid_full,
                full_version,
                hash_algorithm,
                observed_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_content_fingerprint(
        &self,
        record: &ContentFingerprintRecord,
    ) -> Result<(), IndexError> {
        let path_str = record.canonical_path.to_string_lossy();

        let existing: Option<ExistingFingerprintRow> = self
            .conn
            .query_row(
                r#"
            SELECT logical_size, mtime_sec, mtime_nsec, ctime_sec, ctime_nsec,
                   sample_hash, sample_version, full_hash, full_version
            FROM content_fingerprints
            WHERE device_id = ?1 AND inode = ?2 AND canonical_path = ?3
            "#,
                rusqlite::params![record.device_id, record.inode, path_str.as_ref()],
                |row| {
                    Ok(ExistingFingerprintRow {
                        logical_size: row.get(0)?,
                        mtime_sec: row.get(1)?,
                        mtime_nsec: row.get(2)?,
                        ctime_sec: row.get(3)?,
                        ctime_nsec: row.get(4)?,
                        sample_hash: row.get(5)?,
                        sample_version: row.get(6)?,
                        full_hash: row.get(7)?,
                        full_version: row.get(8)?,
                    })
                },
            )
            .optional()?;

        match existing {
            Some(ex) => {
                let identity_matches = ex.logical_size == record.logical_size
                    && ex.mtime_sec == record.mtime_sec
                    && ex.mtime_nsec == record.mtime_nsec
                    && ex.ctime_sec == record.ctime_sec
                    && ex.ctime_nsec == record.ctime_nsec;

                let (final_shash, final_sver, final_fhash, final_fver) = if identity_matches {
                    let s_hash = record.sample_hash.clone().or(ex.sample_hash);
                    let s_ver = if record.sample_hash.is_some() {
                        record.sample_version.clone()
                    } else {
                        ex.sample_version
                    };
                    let f_hash = record.full_hash.clone().or(ex.full_hash);
                    let f_ver = if record.full_hash.is_some() {
                        record.full_version.clone()
                    } else {
                        ex.full_version
                    };
                    (s_hash, s_ver, f_hash, f_ver)
                } else {
                    // Identity changed: clear stale sample/full hashes completely
                    (
                        record.sample_hash.clone(),
                        record.sample_version.clone(),
                        record.full_hash.clone(),
                        record.full_version.clone(),
                    )
                };

                self.conn.execute(
                    r#"
                    UPDATE content_fingerprints SET
                        logical_size = ?1,
                        mtime_sec = ?2,
                        mtime_nsec = ?3,
                        ctime_sec = ?4,
                        ctime_nsec = ?5,
                        sample_hash = ?6,
                        sample_version = ?7,
                        full_hash = ?8,
                        full_version = ?9,
                        hash_algorithm = ?10,
                        observed_at = ?11
                    WHERE device_id = ?12 AND inode = ?13 AND canonical_path = ?14
                    "#,
                    rusqlite::params![
                        record.logical_size,
                        record.mtime_sec,
                        record.mtime_nsec,
                        record.ctime_sec,
                        record.ctime_nsec,
                        final_shash,
                        final_sver,
                        final_fhash,
                        final_fver,
                        record.hash_algorithm,
                        record.observed_at,
                        record.device_id,
                        record.inode,
                        path_str.as_ref(),
                    ],
                )?;
            }
            None => {
                let fp_ver = record
                    .full_version
                    .as_deref()
                    .or(record.sample_version.as_deref())
                    .unwrap_or("");
                self.conn.execute(
                    r#"
                    INSERT INTO content_fingerprints (
                        device_id, inode, canonical_path, logical_size,
                        mtime_sec, mtime_nsec, ctime_sec, ctime_nsec,
                        sample_hash, sample_version, full_hash, full_version,
                        hash_algorithm, fingerprint_version, observed_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                    "#,
                    rusqlite::params![
                        record.device_id,
                        record.inode,
                        path_str.as_ref(),
                        record.logical_size,
                        record.mtime_sec,
                        record.mtime_nsec,
                        record.ctime_sec,
                        record.ctime_nsec,
                        record.sample_hash,
                        record.sample_version,
                        record.full_hash,
                        record.full_version,
                        record.hash_algorithm,
                        fp_ver,
                        record.observed_at,
                    ],
                )?;
            }
        }
        Ok(())
    }

    pub fn get_fingerprint_stats(&self) -> Result<FingerprintCacheStats, IndexError> {
        let total: u64 = self
            .conn
            .query_row("SELECT count(*) FROM content_fingerprints", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        let sample_count: u64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM content_fingerprints WHERE sample_hash IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let full_count: u64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM content_fingerprints WHERE full_hash IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        Ok(FingerprintCacheStats {
            total_entries: total,
            sample_hash_count: sample_count,
            full_hash_count: full_count,
        })
    }

    pub fn prune_missing_fingerprints(&self) -> Result<usize, IndexError> {
        let mut stmt = self
            .conn
            .prepare("SELECT canonical_path FROM content_fingerprints")?;
        let rows = stmt.query_map([], |row| {
            let path_str: String = row.get(0)?;
            Ok(path_str)
        })?;

        let mut to_delete = Vec::new();
        for item in rows {
            let path_str = item?;
            let path = PathBuf::from(&path_str);
            if !path.exists() {
                to_delete.push(path_str);
            }
        }

        let deleted_count = to_delete.len();
        if !to_delete.is_empty() {
            let tx = self.conn.unchecked_transaction()?;
            for p in to_delete {
                tx.execute(
                    "DELETE FROM content_fingerprints WHERE canonical_path = ?1",
                    rusqlite::params![p],
                )?;
            }
            tx.commit()?;
        }

        Ok(deleted_count)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFingerprint {
    pub sample_hash: Option<String>,
    pub sample_version: Option<String>,
    pub full_hash: Option<String>,
    pub full_version: Option<String>,
    pub hash_algorithm: String,
    pub observed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFingerprintRecord {
    pub device_id: u64,
    pub inode: u64,
    pub canonical_path: PathBuf,
    pub logical_size: u64,
    pub mtime_sec: i64,
    pub mtime_nsec: i64,
    pub ctime_sec: i64,
    pub ctime_nsec: i64,
    pub sample_hash: Option<String>,
    pub sample_version: Option<String>,
    pub full_hash: Option<String>,
    pub full_version: Option<String>,
    pub hash_algorithm: String,
    pub observed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FingerprintCacheStats {
    pub total_entries: u64,
    pub sample_hash_count: u64,
    pub full_hash_count: u64,
}

/// Aggregates recursive physical allocated bytes, logical bytes, and file/dir counts for each directory in a scan.
pub fn build_recursive_subtrees(
    root: &Path,
    entries: &[ScannedEntry],
) -> HashMap<String, SnapshotSubtreeStats> {
    let mut dir_stats: HashMap<PathBuf, SnapshotSubtreeStats> = HashMap::new();

    dir_stats.insert(
        root.to_path_buf(),
        SnapshotSubtreeStats {
            logical_bytes: 0,
            allocated_bytes: 0,
            file_count: 0,
            dir_count: 0,
        },
    );

    // 1. Register all directories
    for entry in entries {
        if entry.is_dir {
            dir_stats
                .entry(entry.path.clone())
                .or_insert_with(|| SnapshotSubtreeStats {
                    logical_bytes: 0,
                    allocated_bytes: 0,
                    file_count: 0,
                    dir_count: 0,
                });
        }
    }

    // 2. Direct assignment of non-directories and immediate child dirs
    for entry in entries {
        if !entry.is_dir {
            if let Some(parent) = entry.path.parent() {
                if let Some(p_stats) = dir_stats.get_mut(parent) {
                    p_stats.allocated_bytes = p_stats
                        .allocated_bytes
                        .saturating_add(entry.allocated_bytes);
                    p_stats.logical_bytes =
                        p_stats.logical_bytes.saturating_add(entry.logical_bytes);
                    p_stats.file_count = p_stats.file_count.saturating_add(1);
                }
            }
        } else if entry.path != root {
            if let Some(parent) = entry.path.parent() {
                if let Some(p_stats) = dir_stats.get_mut(parent) {
                    p_stats.dir_count = p_stats.dir_count.saturating_add(1);
                }
            }
        }
    }

    // 3. Roll up bottom-up (deepest directory paths first)
    let mut dirs_by_depth: Vec<PathBuf> = dir_stats.keys().cloned().collect();
    dirs_by_depth.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

    for d in dirs_by_depth {
        if d == root {
            continue;
        }
        let child_stats = dir_stats.get(&d).cloned().unwrap();
        if let Some(parent) = d.parent() {
            if let Some(p_stats) = dir_stats.get_mut(parent) {
                p_stats.allocated_bytes = p_stats
                    .allocated_bytes
                    .saturating_add(child_stats.allocated_bytes);
                p_stats.logical_bytes = p_stats
                    .logical_bytes
                    .saturating_add(child_stats.logical_bytes);
                p_stats.file_count = p_stats.file_count.saturating_add(child_stats.file_count);
                p_stats.dir_count = p_stats.dir_count.saturating_add(child_stats.dir_count);
            }
        }
    }

    dir_stats
        .into_iter()
        .map(|(p, s)| (p.to_string_lossy().to_string(), s))
        .collect()
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
            kernel_private_bytes: None,
            inode: 101,
            device_id: 1,
            file_kind: vacua_core::fs::FileKind::Regular,
            is_dir: false,
            is_symlink: false,
            is_sparse: false,
            is_clone: false,
            clone_id: None,
            clone_refcnt: 1,
            nlink: 1,
            mtime_sec: 1000,
            mtime_nsec: 0,
            ctime_sec: 1000,
            ctime_nsec: 0,
        };

        let inserted = db.upsert_entries(&[scanned]).unwrap();
        assert_eq!(inserted, 1);

        let stats = db.get_index_stats().unwrap();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.total_logical_bytes, 5000);
        assert_eq!(stats.total_allocated_bytes, 8192);
    }

    #[test]
    fn test_watched_roots_persistence() {
        let mut db = IndexDatabase::open_in_memory().unwrap();
        let path = PathBuf::from("/Users/test/workspace");
        db.upsert_watched_root(&path, 1, 123456, "active").unwrap();

        let cur = db.get_watched_root(&path).unwrap().unwrap();
        assert_eq!(cur.watched_root, path);
        assert_eq!(cur.last_event_id, 123456);
        assert_eq!(cur.status, "active");

        let list = db.list_watched_roots().unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_storage_snapshot_and_diff() {
        let mut db = IndexDatabase::open_in_memory().unwrap();

        let mut sub1 = HashMap::new();
        sub1.insert(
            "~/Library/Developer".to_string(),
            SnapshotSubtreeStats {
                logical_bytes: 8 * 1024 * 1024,
                allocated_bytes: 10 * 1024 * 1024,
                file_count: 50,
                dir_count: 5,
            },
        );
        sub1.insert(
            "~/Library/Caches".to_string(),
            SnapshotSubtreeStats {
                logical_bytes: 4 * 1024 * 1024,
                allocated_bytes: 5 * 1024 * 1024,
                file_count: 20,
                dir_count: 2,
            },
        );

        let s1 = StorageSnapshot {
            snapshot_id: "snap-1".into(),
            name: "baseline".into(),
            root_path: PathBuf::from("/Users/test"),
            timestamp: 1000,
            total_files: 100,
            total_dirs: 10,
            logical_bytes: 15 * 1024 * 1024,
            allocated_bytes: 15 * 1024 * 1024,
            subtrees: sub1,
        };
        db.save_snapshot(&s1).unwrap();

        let mut sub2 = HashMap::new();
        sub2.insert(
            "~/Library/Developer".to_string(),
            SnapshotSubtreeStats {
                logical_bytes: 15 * 1024 * 1024,
                allocated_bytes: 18 * 1024 * 1024, // +8 MB
                file_count: 90,
                dir_count: 6,
            },
        );
        sub2.insert(
            "~/Library/Caches".to_string(),
            SnapshotSubtreeStats {
                logical_bytes: 1024 * 1024,
                allocated_bytes: 2 * 1024 * 1024, // -3 MB
                file_count: 10,
                dir_count: 2,
            },
        );
        sub2.insert(
            "~/Docker".to_string(),
            SnapshotSubtreeStats {
                logical_bytes: 3 * 1024 * 1024,
                allocated_bytes: 4 * 1024 * 1024, // +4 MB (created)
                file_count: 25,
                dir_count: 3,
            },
        );

        let s2 = StorageSnapshot {
            snapshot_id: "snap-2".into(),
            name: "current".into(),
            root_path: PathBuf::from("/Users/test"),
            timestamp: 2000,
            total_files: 150,
            total_dirs: 12,
            logical_bytes: 24 * 1024 * 1024,
            allocated_bytes: 24 * 1024 * 1024,
            subtrees: sub2,
        };
        db.save_snapshot(&s2).unwrap();

        let diff = db.diff_snapshots("baseline", "current").unwrap();
        assert_eq!(diff.allocated_delta_bytes, 9 * 1024 * 1024);
        assert_eq!(diff.files_delta, 50);

        let dev_delta = diff
            .subtree_deltas
            .iter()
            .find(|d| d.path == "~/Library/Developer")
            .unwrap();
        assert_eq!(dev_delta.delta_bytes, 8 * 1024 * 1024);
        assert_eq!(dev_delta.change_type, "grown");

        let docker_delta = diff
            .subtree_deltas
            .iter()
            .find(|d| d.path == "~/Docker")
            .unwrap();
        assert_eq!(docker_delta.delta_bytes, 4 * 1024 * 1024);
        assert_eq!(docker_delta.change_type, "created");
    }

    #[test]
    fn test_recursive_subtree_aggregation() {
        let root = PathBuf::from("/test_root");
        let dir_a = root.join("A");
        let dir_b = root.join("B");
        let dir_a_sub = dir_a.join("sub");

        let entries = vec![
            ScannedEntry {
                path: dir_a.clone(),
                logical_bytes: 0,
                allocated_bytes: 4096,
                kernel_private_bytes: None,
                inode: 1,
                device_id: 1,
                file_kind: vacua_core::fs::FileKind::Directory,
                is_dir: true,
                is_symlink: false,
                is_sparse: false,
                is_clone: false,
                clone_id: None,
                clone_refcnt: 1,
                nlink: 2,
                mtime_sec: 100,
                mtime_nsec: 0,
                ctime_sec: 100,
                ctime_nsec: 0,
            },
            ScannedEntry {
                path: dir_a_sub.clone(),
                logical_bytes: 0,
                allocated_bytes: 4096,
                kernel_private_bytes: None,
                inode: 2,
                device_id: 1,
                file_kind: vacua_core::fs::FileKind::Directory,
                is_dir: true,
                is_symlink: false,
                is_sparse: false,
                is_clone: false,
                clone_id: None,
                clone_refcnt: 1,
                nlink: 2,
                mtime_sec: 100,
                mtime_nsec: 0,
                ctime_sec: 100,
                ctime_nsec: 0,
            },
            ScannedEntry {
                path: dir_a_sub.join("file1.bin"),
                logical_bytes: 10 * 1024 * 1024,
                allocated_bytes: 10 * 1024 * 1024,
                kernel_private_bytes: None,
                inode: 3,
                device_id: 1,
                file_kind: vacua_core::fs::FileKind::Regular,
                is_dir: false,
                is_symlink: false,
                is_sparse: false,
                is_clone: false,
                clone_id: None,
                clone_refcnt: 1,
                nlink: 1,
                mtime_sec: 100,
                mtime_nsec: 0,
                ctime_sec: 100,
                ctime_nsec: 0,
            },
            ScannedEntry {
                path: dir_b.clone(),
                logical_bytes: 0,
                allocated_bytes: 4096,
                kernel_private_bytes: None,
                inode: 4,
                device_id: 1,
                file_kind: vacua_core::fs::FileKind::Directory,
                is_dir: true,
                is_symlink: false,
                is_sparse: false,
                is_clone: false,
                clone_id: None,
                clone_refcnt: 1,
                nlink: 2,
                mtime_sec: 100,
                mtime_nsec: 0,
                ctime_sec: 100,
                ctime_nsec: 0,
            },
            ScannedEntry {
                path: dir_b.join("file2.bin"),
                logical_bytes: 5 * 1024 * 1024,
                allocated_bytes: 5 * 1024 * 1024,
                kernel_private_bytes: None,
                inode: 5,
                device_id: 1,
                file_kind: vacua_core::fs::FileKind::Regular,
                is_dir: false,
                is_symlink: false,
                is_sparse: false,
                is_clone: false,
                clone_id: None,
                clone_refcnt: 1,
                nlink: 1,
                mtime_sec: 100,
                mtime_nsec: 0,
                ctime_sec: 100,
                ctime_nsec: 0,
            },
        ];

        let subtrees = build_recursive_subtrees(&root, &entries);

        let a_sub_stat = subtrees
            .get(&dir_a_sub.to_string_lossy().to_string())
            .unwrap();
        assert_eq!(a_sub_stat.file_count, 1);
        assert_eq!(a_sub_stat.allocated_bytes, 10 * 1024 * 1024);

        let a_stat = subtrees.get(&dir_a.to_string_lossy().to_string()).unwrap();
        assert_eq!(a_stat.file_count, 1);
        assert_eq!(a_stat.dir_count, 1);
        assert_eq!(a_stat.allocated_bytes, 10 * 1024 * 1024);

        let b_stat = subtrees.get(&dir_b.to_string_lossy().to_string()).unwrap();
        assert_eq!(b_stat.file_count, 1);
        assert_eq!(b_stat.allocated_bytes, 5 * 1024 * 1024);

        let root_stat = subtrees.get(&root.to_string_lossy().to_string()).unwrap();
        assert_eq!(root_stat.file_count, 2);
        assert_eq!(root_stat.allocated_bytes, 15 * 1024 * 1024);
    }

    #[test]
    fn test_fsevents_reconciliation_exact_equivalence() {
        use tempfile::tempdir;
        let tmp = tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();

        // 1. Initial directory structure
        let dir_a = root.join("alpha");
        let dir_b = root.join("beta");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        let f1 = dir_a.join("one.txt");
        let f2 = dir_b.join("two.txt");
        std::fs::write(&f1, b"hello world").unwrap();
        std::fs::write(&f2, b"beta file content").unwrap();

        let mut db = IndexDatabase::open_in_memory().unwrap();
        let scanner = vacua_scan::FilesystemScanner::new(vacua_scan::ScanOptions::default());

        // Full initial scan & index
        let initial_report = scanner.scan(&root).unwrap();
        db.upsert_entries(&initial_report.entries).unwrap();
        let dev_id = initial_report
            .entries
            .first()
            .map(|e| e.device_id)
            .unwrap_or(1);
        db.upsert_watched_root(&root, dev_id, 100, "active")
            .unwrap();

        // 2. Perform filesystem mutations: delete f1, rename f2 to f3, add f4
        std::fs::remove_file(&f1).unwrap();
        let f3 = dir_b.join("three.txt");
        std::fs::rename(&f2, &f3).unwrap();
        let f4 = dir_a.join("four.txt");
        std::fs::write(&f4, b"new file in alpha").unwrap();

        // 3. Incremental refresh using dirty roots
        let dirty_paths = vec![dir_a.clone(), dir_b.clone()];
        db.reconcile_subtrees(&root, &dirty_paths).unwrap();

        // 4. Verify incremental DB rows == fresh full scan entries
        let fresh_report = scanner.scan(&root).unwrap();
        let mut fresh_paths: Vec<String> = fresh_report
            .entries
            .iter()
            .map(|e| e.path.to_string_lossy().to_string())
            .collect();
        fresh_paths.sort();

        let mut db_stmt = db
            .conn
            .prepare("SELECT canonical_path FROM entries WHERE canonical_path LIKE ?1 ORDER BY canonical_path ASC")
            .unwrap();
        let pattern = format!("{}%", root.to_string_lossy());
        let db_rows: Vec<String> = db_stmt
            .query_map(params![pattern], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(
            db_rows, fresh_paths,
            "Incremental DB entries must match fresh full scan"
        );
    }

    #[test]
    fn test_fingerprint_cache_merge_and_invalidation() {
        let db = IndexDatabase::open_in_memory().unwrap();
        let path = PathBuf::from("/test/file.dat");

        // 1. Put sample hash first
        let rec_sample = ContentFingerprintRecord {
            device_id: 1,
            inode: 10,
            canonical_path: path.clone(),
            logical_size: 1024,
            mtime_sec: 100,
            mtime_nsec: 200,
            ctime_sec: 300,
            ctime_nsec: 400,
            sample_hash: Some("sample_abc".to_string()),
            sample_version: Some("vacua-sample-v1".to_string()),
            full_hash: None,
            full_version: None,
            hash_algorithm: "BLAKE3".to_string(),
            observed_at: 1000,
        };
        db.upsert_content_fingerprint(&rec_sample).unwrap();

        // 2. Put full hash next with same identity
        let rec_full = ContentFingerprintRecord {
            device_id: 1,
            inode: 10,
            canonical_path: path.clone(),
            logical_size: 1024,
            mtime_sec: 100,
            mtime_nsec: 200,
            ctime_sec: 300,
            ctime_nsec: 400,
            sample_hash: None,
            sample_version: None,
            full_hash: Some("full_xyz".to_string()),
            full_version: Some("blake3-full-v1".to_string()),
            hash_algorithm: "BLAKE3".to_string(),
            observed_at: 1001,
        };
        db.upsert_content_fingerprint(&rec_full).unwrap();

        // Query: Both sample and full MUST be preserved!
        let cached = db
            .get_content_fingerprint(
                1,
                10,
                &path,
                1024,
                100,
                200,
                300,
                400,
                Some("vacua-sample-v1"),
                Some("blake3-full-v1"),
            )
            .unwrap()
            .unwrap();
        assert_eq!(cached.sample_hash.as_deref(), Some("sample_abc"));
        assert_eq!(cached.full_hash.as_deref(), Some("full_xyz"));

        // 3. Test version mismatch yields miss
        let cached_mismatch = db
            .get_content_fingerprint(
                1,
                10,
                &path,
                1024,
                100,
                200,
                300,
                400,
                Some("vacua-sample-v2"), // Mismatched sample version!
                Some("blake3-full-v1"),
            )
            .unwrap()
            .unwrap();
        assert!(cached_mismatch.sample_hash.is_none());
        assert_eq!(cached_mismatch.full_hash.as_deref(), Some("full_xyz"));

        // 4. File identity changes (mtime modifies) -> Both hashes must become stale!
        let rec_modified = ContentFingerprintRecord {
            device_id: 1,
            inode: 10,
            canonical_path: path.clone(),
            logical_size: 1024,
            mtime_sec: 101, // Changed!
            mtime_nsec: 200,
            ctime_sec: 300,
            ctime_nsec: 400,
            sample_hash: Some("new_sample".to_string()),
            sample_version: Some("vacua-sample-v1".to_string()),
            full_hash: None,
            full_version: None,
            hash_algorithm: "BLAKE3".to_string(),
            observed_at: 1002,
        };
        db.upsert_content_fingerprint(&rec_modified).unwrap();

        // Old full hash must be gone because identity changed!
        let cached_new = db
            .get_content_fingerprint(
                1,
                10,
                &path,
                1024,
                101,
                200,
                300,
                400,
                Some("vacua-sample-v1"),
                Some("blake3-full-v1"),
            )
            .unwrap()
            .unwrap();
        assert_eq!(cached_new.sample_hash.as_deref(), Some("new_sample"));
        assert!(cached_new.full_hash.is_none());
    }
}
