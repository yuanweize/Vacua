use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use thiserror::Error;

use vacua_core::invariants::is_protected_path;
use vacua_core::risk::RiskLevel;
use vacua_plan::CleanupPlan;

use crate::backend::{BackendError, TrashBackend};
use crate::journal::{ExecutionJournal, JournalError};

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Plan integrity hash verification failed (possible tampering)")]
    PlanHashMismatch,

    #[error("Plan contains invalid or corrupted items")]
    InvalidPlan(String),

    #[error("I/O error during execution: {0}")]
    Io(#[from] std::io::Error),

    #[error("Trash backend error: {0}")]
    Backend(#[from] BackendError),

    #[error("Execution journal error: {0}")]
    Journal(#[from] JournalError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutedItem {
    pub candidate_id: String,
    pub path: PathBuf,
    pub risk: RiskLevel,
    pub allocated_bytes: u64,
    pub destination: Option<PathBuf>,
    pub reversible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedItem {
    pub candidate_id: String,
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedItem {
    pub candidate_id: String,
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub transaction_id: String,
    pub plan_hash: String,
    pub dry_run: bool,
    pub total_planned_items: usize,
    pub successful_items: Vec<ExecutedItem>,
    pub skipped_items: Vec<SkippedItem>,
    pub failed_items: Vec<FailedItem>,
    pub total_reclaimed_bytes: u64,
}

pub struct PlanExecutor<'a, B: TrashBackend> {
    backend: &'a B,
    dry_run: bool,
}

impl<'a, B: TrashBackend> PlanExecutor<'a, B> {
    pub fn new(backend: &'a B, dry_run: bool) -> Self {
        Self { backend, dry_run }
    }

    pub fn execute(
        &self,
        plan: &CleanupPlan,
        mut journal: Option<&mut ExecutionJournal>,
    ) -> Result<ExecutionReport, ExecutorError> {
        // Step 1: Verify plan cryptographic integrity
        if plan.verify_integrity().is_err() {
            return Err(ExecutorError::PlanHashMismatch);
        }

        let transaction_id = format!("tx-{}", Utc::now().timestamp_millis());
        let mut successful = Vec::new();
        let mut skipped = Vec::new();
        let mut failed = Vec::new();
        let mut total_reclaimed = 0u64;

        // Query active processes for running guards
        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );

        for item in &plan.items {
            // Invariant Gate 1: Never touch protected or unknown items
            if item.risk == RiskLevel::Protected
                || item.risk == RiskLevel::Unknown
                || is_protected_path(&item.path)
            {
                let reason = "Safety invariant violation: item is protected or unknown".to_string();
                if let Some(ref mut j) = journal {
                    j.record(
                        &transaction_id,
                        &plan.plan_hash,
                        &item.candidate_id,
                        &item.path.to_string_lossy(),
                        "skip",
                        &format!("{:?}", item.risk),
                        "invariant_guard",
                        Some(item.expected_inode),
                        None,
                        "skipped",
                        0,
                        true,
                        Some(&reason),
                    )?;
                }
                skipped.push(SkippedItem {
                    candidate_id: item.candidate_id.clone(),
                    path: item.path.clone(),
                    reason,
                });
                continue;
            }

            // TOCTOU Gate 2: Live filesystem inspection using lstat / symlink_metadata
            let live_meta = match std::fs::symlink_metadata(&item.path) {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let reason = "Target path no longer exists on live filesystem".to_string();
                    if let Some(ref mut j) = journal {
                        j.record(
                            &transaction_id,
                            &plan.plan_hash,
                            &item.candidate_id,
                            &item.path.to_string_lossy(),
                            "skip",
                            &format!("{:?}", item.risk),
                            "toctou_exists",
                            Some(item.expected_inode),
                            None,
                            "skipped",
                            0,
                            true,
                            Some(&reason),
                        )?;
                    }
                    skipped.push(SkippedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        reason,
                    });
                    continue;
                }
                Err(e) => {
                    let err_msg = format!("Failed to read metadata: {}", e);
                    failed.push(FailedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        error: err_msg,
                    });
                    continue;
                }
            };

            // Device ID check
            if live_meta.dev() != item.expected_device_id {
                let reason = format!(
                    "TOCTOU device ID mismatch (expected {}, live {})",
                    item.expected_device_id,
                    live_meta.dev()
                );
                if let Some(ref mut j) = journal {
                    j.record(
                        &transaction_id,
                        &plan.plan_hash,
                        &item.candidate_id,
                        &item.path.to_string_lossy(),
                        "skip",
                        &format!("{:?}", item.risk),
                        "toctou_device",
                        Some(item.expected_inode),
                        Some(live_meta.ino()),
                        "skipped",
                        0,
                        true,
                        Some(&reason),
                    )?;
                }
                skipped.push(SkippedItem {
                    candidate_id: item.candidate_id.clone(),
                    path: item.path.clone(),
                    reason,
                });
                continue;
            }

            // Inode check (prevents hardlink substitution or target replacement)
            if live_meta.ino() != item.expected_inode {
                let reason = format!(
                    "TOCTOU inode mismatch: file was replaced or recreated (expected {}, live {})",
                    item.expected_inode,
                    live_meta.ino()
                );
                if let Some(ref mut j) = journal {
                    j.record(
                        &transaction_id,
                        &plan.plan_hash,
                        &item.candidate_id,
                        &item.path.to_string_lossy(),
                        "skip",
                        &format!("{:?}", item.risk),
                        "toctou_inode",
                        Some(item.expected_inode),
                        Some(live_meta.ino()),
                        "skipped",
                        0,
                        true,
                        Some(&reason),
                    )?;
                }
                skipped.push(SkippedItem {
                    candidate_id: item.candidate_id.clone(),
                    path: item.path.clone(),
                    reason,
                });
                continue;
            }

            // Modification time check (prevents deleting newly modified items)
            if live_meta.mtime() != item.expected_mtime_sec {
                let reason = format!(
                    "TOCTOU mtime mismatch: target modified after plan compilation (expected {}, live {})",
                    item.expected_mtime_sec,
                    live_meta.mtime()
                );
                if let Some(ref mut j) = journal {
                    j.record(
                        &transaction_id,
                        &plan.plan_hash,
                        &item.candidate_id,
                        &item.path.to_string_lossy(),
                        "skip",
                        &format!("{:?}", item.risk),
                        "toctou_mtime",
                        Some(item.expected_inode),
                        Some(live_meta.ino()),
                        "skipped",
                        0,
                        true,
                        Some(&reason),
                    )?;
                }
                skipped.push(SkippedItem {
                    candidate_id: item.candidate_id.clone(),
                    path: item.path.clone(),
                    reason,
                });
                continue;
            }

            // Process Guard Gate 3: Check if target relates to active processes
            if let Some(guard_process) = get_target_process_guard(&item.path) {
                let is_running = sys.processes().values().any(|p| {
                    p.name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(guard_process)
                });

                if is_running {
                    let reason = format!(
                        "Active process guard triggered: '{}' is currently running",
                        guard_process
                    );
                    if let Some(ref mut j) = journal {
                        j.record(
                            &transaction_id,
                            &plan.plan_hash,
                            &item.candidate_id,
                            &item.path.to_string_lossy(),
                            "skip",
                            &format!("{:?}", item.risk),
                            "process_guard",
                            Some(item.expected_inode),
                            Some(live_meta.ino()),
                            "skipped",
                            0,
                            true,
                            Some(&reason),
                        )?;
                    }
                    skipped.push(SkippedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        reason,
                    });
                    continue;
                }
            }

            // Dry-run mode: do not mutate filesystem
            if self.dry_run {
                if let Some(ref mut j) = journal {
                    j.record(
                        &transaction_id,
                        &plan.plan_hash,
                        &item.candidate_id,
                        &item.path.to_string_lossy(),
                        "trash (dry_run)",
                        &format!("{:?}", item.risk),
                        "plan_item",
                        Some(item.expected_inode),
                        Some(live_meta.ino()),
                        "dry_run",
                        item.allocated_bytes,
                        true,
                        None,
                    )?;
                }
                total_reclaimed += item.allocated_bytes;
                successful.push(ExecutedItem {
                    candidate_id: item.candidate_id.clone(),
                    path: item.path.clone(),
                    risk: item.risk,
                    allocated_bytes: item.allocated_bytes,
                    destination: None,
                    reversible: true,
                });
                continue;
            }

            // FAIL-SAFE RULE: Pre-action journal intent MUST be written before touching filesystem.
            // If the journal write fails, abort action immediately to prevent untracked deletions.
            if let Some(ref mut j) = journal {
                j.record(
                    &transaction_id,
                    &plan.plan_hash,
                    &item.candidate_id,
                    &item.path.to_string_lossy(),
                    "trash_intent",
                    &format!("{:?}", item.risk),
                    "pre_action_intent",
                    Some(item.expected_inode),
                    Some(live_meta.ino()),
                    "pending",
                    item.allocated_bytes,
                    true,
                    None,
                )?;
            }

            // Action: move to Trash via backend
            match self.backend.trash(&item.path) {
                Ok(dest) => {
                    total_reclaimed += item.allocated_bytes;
                    if let Some(ref mut j) = journal {
                        j.record(
                            &transaction_id,
                            &plan.plan_hash,
                            &item.candidate_id,
                            &item.path.to_string_lossy(),
                            "trash",
                            &format!("{:?}", item.risk),
                            "plan_item",
                            Some(item.expected_inode),
                            Some(live_meta.ino()),
                            "success",
                            item.allocated_bytes,
                            true,
                            None,
                        )?;
                    }
                    successful.push(ExecutedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        risk: item.risk,
                        allocated_bytes: item.allocated_bytes,
                        destination: Some(dest),
                        reversible: true,
                    });
                }
                Err(e) => {
                    let err_msg = format!("Trash operation failed: {}", e);
                    if let Some(ref mut j) = journal {
                        let _ = j.record(
                            &transaction_id,
                            &plan.plan_hash,
                            &item.candidate_id,
                            &item.path.to_string_lossy(),
                            "trash",
                            &format!("{:?}", item.risk),
                            "plan_item",
                            Some(item.expected_inode),
                            Some(live_meta.ino()),
                            "failed",
                            0,
                            true,
                            Some(&err_msg),
                        );
                    }
                    failed.push(FailedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        error: err_msg,
                    });
                }
            }
        }

        Ok(ExecutionReport {
            transaction_id,
            plan_hash: plan.plan_hash.clone(),
            dry_run: self.dry_run,
            total_planned_items: plan.items.len(),
            successful_items: successful,
            skipped_items: skipped,
            failed_items: failed,
            total_reclaimed_bytes: total_reclaimed,
        })
    }
}

/// Identifies if target path is related to an active process guard.
fn get_target_process_guard(path: &Path) -> Option<&'static str> {
    let s = path.to_string_lossy();
    if s.contains("Library/Developer/Xcode/DerivedData") {
        Some("Xcode")
    } else if s.contains("Library/Caches/com.docker.docker")
        || s.contains(".docker")
        || s.contains("OrbStack")
    {
        Some("Docker")
    } else if s.contains("Library/Caches/Google/Chrome") {
        Some("Google Chrome")
    } else {
        None
    }
}
