use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JournalError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalRecord {
    pub id: i64,
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

pub struct ExecutionJournal {
    conn: Connection,
}

impl ExecutionJournal {
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

        Ok(Self { conn })
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
        let now = Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO execution_journal (
                transaction_id, plan_hash, timestamp, candidate_id, path, action,
                risk, rule, expected_inode, actual_inode, result, reclaimed_estimate,
                reversible, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
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
            "SELECT id, transaction_id, plan_hash, timestamp, candidate_id, path, action,
                    risk, rule, expected_inode, actual_inode, result, reclaimed_estimate,
                    reversible, error_message
             FROM execution_journal
             WHERE transaction_id = ?1
             ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![tx_id], |row| {
            let exp_ino: Option<i64> = row.get(9)?;
            let act_ino: Option<i64> = row.get(10)?;
            let reclaimed: i64 = row.get(12)?;

            Ok(JournalRecord {
                id: row.get(0)?,
                transaction_id: row.get(1)?,
                plan_hash: row.get(2)?,
                timestamp: row.get(3)?,
                candidate_id: row.get(4)?,
                path: row.get(5)?,
                action: row.get(6)?,
                risk: row.get(7)?,
                rule: row.get(8)?,
                expected_inode: exp_ino.map(|i| i as u64),
                actual_inode: act_ino.map(|i| i as u64),
                result: row.get(11)?,
                reclaimed_estimate: reclaimed.max(0) as u64,
                reversible: row.get(13)?,
                error_message: row.get(14)?,
            })
        })?;

        let mut records = Vec::new();
        for r in rows {
            records.push(r?);
        }
        Ok(records)
    }
}
