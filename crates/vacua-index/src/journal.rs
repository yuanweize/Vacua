use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;

pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Error, Debug)]
pub enum JournalError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Audit log tampering detected at record ID {record_id}: {reason}")]
    Tampered { record_id: i64, reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalRecord {
    pub id: i64,
    pub prev_hash: String,
    pub entry_hash: String,
    pub transaction_id: String,
    pub plan_hash: String,
    pub timestamp: i64,
    pub candidate_id: String,
    pub path: String,
    pub action: String,
    pub risk: String,
    pub rule: String,
    pub expected_inode: Option<u64>,
    pub actual_inode: Option<u64>,
    pub result: String,
    pub reclaimed_estimate: u64,
    pub reversible: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub transaction_id: String,
    pub plan_hash: String,
    pub timestamp: i64,
    pub total_items: usize,
    pub total_reclaimed_bytes: u64,
    pub successful_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub total_records: usize,
    pub is_valid: bool,
    pub first_hash: Option<String>,
    pub latest_hash: Option<String>,
    pub broken_record_id: Option<i64>,
    pub error_detail: Option<String>,
}

pub struct ExecutionJournal {
    conn: Connection,
}

#[allow(clippy::too_many_arguments)]
fn compute_entry_hash(
    prev_hash: &str,
    transaction_id: &str,
    plan_hash: &str,
    timestamp: i64,
    candidate_id: &str,
    path: &str,
    action: &str,
    result: &str,
    reclaimed_estimate: u64,
) -> String {
    let mut hasher = Sha256::new();
    let payload = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        prev_hash,
        transaction_id,
        plan_hash,
        timestamp,
        candidate_id,
        path,
        action,
        result,
        reclaimed_estimate
    );
    hasher.update(payload.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl ExecutionJournal {
    /// Open execution journal strictly read-only without executing DDL, WAL pragma, or creating directories.
    pub fn open_read_only(db_path: &Path) -> Result<Self, JournalError> {
        if !db_path.exists() {
            return Err(JournalError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Journal database not found at {}", db_path.display()),
            )));
        }

        let conn = Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        Ok(Self { conn })
    }

    pub fn open(db_path: &Path) -> Result<Self, JournalError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS execution_journal (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prev_hash TEXT NOT NULL DEFAULT '',
                entry_hash TEXT NOT NULL DEFAULT '',
                transaction_id TEXT NOT NULL,
                plan_hash TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                candidate_id TEXT NOT NULL,
                path TEXT NOT NULL,
                action TEXT NOT NULL,
                risk TEXT NOT NULL,
                rule TEXT NOT NULL,
                expected_inode INTEGER,
                actual_inode INTEGER,
                result TEXT NOT NULL,
                reclaimed_estimate INTEGER NOT NULL,
                reversible BOOLEAN NOT NULL,
                error_message TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_journal_tx ON execution_journal(transaction_id);
            CREATE INDEX IF NOT EXISTS idx_journal_time ON execution_journal(timestamp DESC);",
        )?;

        // Schema migration for v0.1.0 -> v0.2.0: ensure hash chaining columns exist
        let _ = conn.execute(
            "ALTER TABLE execution_journal ADD COLUMN prev_hash TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE execution_journal ADD COLUMN entry_hash TEXT NOT NULL DEFAULT ''",
            [],
        );

        Ok(Self { conn })
    }

    /// Retrieve the entry_hash of the latest record in the hash chain.
    fn get_latest_hash(&self) -> Result<String, JournalError> {
        let mut stmt = self.conn.prepare(
            "SELECT entry_hash FROM execution_journal WHERE entry_hash != '' ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let h: String = row.get(0)?;
            if !h.is_empty() {
                return Ok(h);
            }
        }
        Ok(GENESIS_HASH.to_string())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        transaction_id: &str,
        plan_hash: &str,
        candidate_id: &str,
        path: &str,
        action: &str,
        risk: &str,
        rule: &str,
        expected_inode: Option<u64>,
        actual_inode: Option<u64>,
        result: &str,
        reclaimed_estimate: u64,
        reversible: bool,
        error_message: Option<&str>,
    ) -> Result<i64, JournalError> {
        let prev_hash = self.get_latest_hash()?;
        let now = Utc::now().timestamp();
        let entry_hash = compute_entry_hash(
            &prev_hash,
            transaction_id,
            plan_hash,
            now,
            candidate_id,
            path,
            action,
            result,
            reclaimed_estimate,
        );

        self.conn.execute(
            "INSERT INTO execution_journal (
                prev_hash, entry_hash, transaction_id, plan_hash, timestamp,
                candidate_id, path, action, risk, rule, expected_inode, actual_inode,
                result, reclaimed_estimate, reversible, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                prev_hash,
                entry_hash,
                transaction_id,
                plan_hash,
                now,
                candidate_id,
                path,
                action,
                risk,
                rule,
                expected_inode.map(|i| i as i64),
                actual_inode.map(|i| i as i64),
                result,
                reclaimed_estimate as i64,
                reversible,
                error_message,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Verifies the cryptographic integrity of the entire journal hash chain.
    pub fn verify_chain(&self) -> Result<VerificationReport, JournalError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, prev_hash, entry_hash, transaction_id, plan_hash, timestamp,
                    candidate_id, path, action, result, reclaimed_estimate
             FROM execution_journal
             WHERE entry_hash != ''
             ORDER BY id ASC",
        )?;

        let mut expected_prev = GENESIS_HASH.to_string();
        let mut count = 0;
        let mut first_hash = None;
        let mut latest_hash = None;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
            ))
        })?;

        for r in rows {
            let (
                id,
                prev_hash,
                entry_hash,
                tx_id,
                plan_hash,
                ts,
                cand_id,
                path,
                action,
                res,
                reclaimed,
            ) = r?;

            count += 1;
            if first_hash.is_none() {
                first_hash = Some(entry_hash.clone());
            }
            latest_hash = Some(entry_hash.clone());

            // 1. Verify link to previous entry
            if prev_hash != expected_prev {
                return Ok(VerificationReport {
                    total_records: count,
                    is_valid: false,
                    first_hash,
                    latest_hash,
                    broken_record_id: Some(id),
                    error_detail: Some(format!(
                        "Chain link broken at record ID {}: expected prev_hash {}, found {}",
                        id, expected_prev, prev_hash
                    )),
                });
            }

            // 2. Recompute and verify entry_hash
            let computed = compute_entry_hash(
                &prev_hash,
                &tx_id,
                &plan_hash,
                ts,
                &cand_id,
                &path,
                &action,
                &res,
                reclaimed.max(0) as u64,
            );
            if computed != entry_hash {
                return Ok(VerificationReport {
                    total_records: count,
                    is_valid: false,
                    first_hash,
                    latest_hash,
                    broken_record_id: Some(id),
                    error_detail: Some(format!(
                        "Hash mismatch at record ID {}: computed {}, recorded {}",
                        id, computed, entry_hash
                    )),
                });
            }

            expected_prev = entry_hash;
        }

        Ok(VerificationReport {
            total_records: count,
            is_valid: true,
            first_hash,
            latest_hash,
            broken_record_id: None,
            error_detail: None,
        })
    }

    pub fn list_transactions(&self, limit: usize) -> Result<Vec<TransactionSummary>, JournalError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                transaction_id,
                plan_hash,
                max(timestamp) as ts,
                count(*) as total_items,
                sum(CASE WHEN result = 'success' THEN reclaimed_estimate ELSE 0 END) as reclaimed,
                sum(CASE WHEN result = 'success' THEN 1 ELSE 0 END) as success_cnt,
                sum(CASE WHEN result = 'skipped' THEN 1 ELSE 0 END) as skipped_cnt,
                sum(CASE WHEN result = 'failed' THEN 1 ELSE 0 END) as failed_cnt
             FROM execution_journal
             GROUP BY transaction_id, plan_hash
             ORDER BY ts DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let total_items: i64 = row.get(3)?;
            let reclaimed: i64 = row.get(4)?;
            let success_cnt: i64 = row.get(5)?;
            let skipped_cnt: i64 = row.get(6)?;
            let failed_cnt: i64 = row.get(7)?;

            Ok(TransactionSummary {
                transaction_id: row.get(0)?,
                plan_hash: row.get(1)?,
                timestamp: row.get(2)?,
                total_items: total_items as usize,
                total_reclaimed_bytes: reclaimed.max(0) as u64,
                successful_count: success_cnt as usize,
                skipped_count: skipped_cnt as usize,
                failed_count: failed_cnt as usize,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_transaction_details(&self, tx_id: &str) -> Result<Vec<JournalRecord>, JournalError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, prev_hash, entry_hash, transaction_id, plan_hash, timestamp, candidate_id, path, action,
                    risk, rule, expected_inode, actual_inode, result, reclaimed_estimate,
                    reversible, error_message
             FROM execution_journal
             WHERE transaction_id = ?1
             ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![tx_id], |row| {
            let exp_ino: Option<i64> = row.get(11)?;
            let act_ino: Option<i64> = row.get(12)?;
            let reclaimed: i64 = row.get(14)?;

            Ok(JournalRecord {
                id: row.get(0)?,
                prev_hash: row.get(1)?,
                entry_hash: row.get(2)?,
                transaction_id: row.get(3)?,
                plan_hash: row.get(4)?,
                timestamp: row.get(5)?,
                candidate_id: row.get(6)?,
                path: row.get(7)?,
                action: row.get(8)?,
                risk: row.get(9)?,
                rule: row.get(10)?,
                expected_inode: exp_ino.map(|i| i as u64),
                actual_inode: act_ino.map(|i| i as u64),
                result: row.get(13)?,
                reclaimed_estimate: reclaimed.max(0) as u64,
                reversible: row.get(15)?,
                error_message: row.get(16)?,
            })
        })?;

        let mut records = Vec::new();
        for r in rows {
            records.push(r?);
        }
        Ok(records)
    }
}
