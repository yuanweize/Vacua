use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use thiserror::Error;

use vacua_core::candidate::CandidateCategory;
use vacua_core::fs::{open_regular_file_safely, query_file_identity, FileIdentity, FileKind};
use vacua_core::invariants::is_protected_path;
use vacua_core::risk::RiskLevel;
use vacua_plan::CleanupPlan;

use crate::backend::{BackendError, TrashBackend};
use crate::journal::{ExecutionJournal, JournalError};

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Plan integrity hash verification failed (possible tampering)")]
    PlanHashMismatch,

    #[error("Plan execution refused: {0}")]
    PlanExecutionRefused(String),

    #[error("Preservation guard failed for '{path}': {reason}")]
    PreservationGuardFailed { path: PathBuf, reason: String },

    #[error("Target content digest mismatch for '{path}': expected {expected}, actual {actual}")]
    TargetContentMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("Plan contains invalid or corrupted items: {0}")]
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
    pub bytes_moved_to_trash: u64,
    pub estimated_eventual_reclaim_after_purge: u64,
    pub immediate_reclaimed_bytes: u64,
    #[serde(default)]
    pub total_reclaimed_bytes: u64,
}

pub struct PlanExecutor<'a, B: TrashBackend> {
    backend: &'a B,
    dry_run: bool,
}

fn compute_file_blake3(file: &std::fs::File) -> std::io::Result<String> {
    let mut f = file;
    (&mut f).seek(SeekFrom::Start(0))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = (&mut f).read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
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

        // Step 2: Verify schema eligibility for execution
        if !self.dry_run {
            plan.verify_for_destructive_execution()
                .map_err(|e| ExecutorError::PlanExecutionRefused(e.to_string()))?;
        }

        // Step 3: Preflight ALL PreservationGuards before any mutation
        for guard in &plan.preservation_guards {
            let file = open_regular_file_safely(&guard.path).map_err(|e| {
                ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!("Cannot safely open preserved file: {}", e),
                }
            })?;
            let ident =
                query_file_identity(&file).map_err(|e| ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!("Failed to query identity of preserved file: {}", e),
                })?;
            if ident.file_kind != FileKind::Regular {
                return Err(ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: "Preserved path is not a regular file".to_string(),
                });
            }
            if ident.device_id != guard.expected_device_id {
                return Err(ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!(
                        "Device mismatch: expected {}, got {}",
                        guard.expected_device_id, ident.device_id
                    ),
                });
            }
            if ident.inode != guard.expected_inode {
                return Err(ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!(
                        "Inode mismatch: expected {}, got {}",
                        guard.expected_inode, ident.inode
                    ),
                });
            }
            if ident.size != guard.expected_size {
                return Err(ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!(
                        "Size mismatch: expected {}, got {}",
                        guard.expected_size, ident.size
                    ),
                });
            }
            if ident.mtime_sec != guard.expected_mtime_sec
                || ident.mtime_nsec != guard.expected_mtime_nsec
            {
                return Err(ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!(
                        "Mtime mismatch: expected {}.{}, got {}.{}",
                        guard.expected_mtime_sec,
                        guard.expected_mtime_nsec,
                        ident.mtime_sec,
                        ident.mtime_nsec
                    ),
                });
            }
            if ident.ctime_sec != guard.expected_ctime_sec
                || ident.ctime_nsec != guard.expected_ctime_nsec
            {
                return Err(ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!(
                        "Ctime mismatch: expected {}.{}, got {}.{}",
                        guard.expected_ctime_sec,
                        guard.expected_ctime_nsec,
                        ident.ctime_sec,
                        ident.ctime_nsec
                    ),
                });
            }
            let digest =
                compute_file_blake3(&file).map_err(|e| ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!("Failed to compute BLAKE3 digest: {}", e),
                })?;
            if digest != guard.full_digest {
                return Err(ExecutorError::PreservationGuardFailed {
                    path: guard.path.clone(),
                    reason: format!(
                        "Content digest mismatch: expected {}, got {}",
                        guard.full_digest, digest
                    ),
                });
            }
        }

        // Step 4: Preflight ALL Target ContentGuards before any mutation
        for item in &plan.items {
            if let Some(ref cg) = item.content_guard {
                let file = open_regular_file_safely(&item.path).map_err(|e| {
                    ExecutorError::TargetContentMismatch {
                        path: item.path.clone(),
                        expected: cg.full_digest.clone(),
                        actual: format!("Cannot safely open target file: {}", e),
                    }
                })?;
                let ident = query_file_identity(&file).map_err(|e| {
                    ExecutorError::TargetContentMismatch {
                        path: item.path.clone(),
                        expected: cg.full_digest.clone(),
                        actual: format!("Failed to query identity of target file: {}", e),
                    }
                })?;
                if ident.file_kind != FileKind::Regular {
                    return Err(ExecutorError::TargetContentMismatch {
                        path: item.path.clone(),
                        expected: cg.full_digest.clone(),
                        actual: "Target path is not a regular file".to_string(),
                    });
                }
                if ident.device_id != item.expected_device_id
                    || ident.inode != item.expected_inode
                    || ident.size != item.expected_size
                    || ident.mtime_sec != item.expected_mtime_sec
                    || ident.mtime_nsec != item.expected_mtime_nsec
                    || ident.ctime_sec != item.expected_ctime_sec
                    || ident.ctime_nsec != item.expected_ctime_nsec
                {
                    return Err(ExecutorError::TargetContentMismatch {
                        path: item.path.clone(),
                        expected: cg.full_digest.clone(),
                        actual: format!(
                            "Identity mismatch (size {} vs {}, mtime {}.{} vs {}.{}, ctime {}.{} vs {}.{})",
                            ident.size,
                            item.expected_size,
                            ident.mtime_sec,
                            ident.mtime_nsec,
                            item.expected_mtime_sec,
                            item.expected_mtime_nsec,
                            ident.ctime_sec,
                            ident.ctime_nsec,
                            item.expected_ctime_sec,
                            item.expected_ctime_nsec,
                        ),
                    });
                }
                let digest = compute_file_blake3(&file).map_err(|e| {
                    ExecutorError::TargetContentMismatch {
                        path: item.path.clone(),
                        expected: cg.full_digest.clone(),
                        actual: format!("Failed to compute BLAKE3 digest: {}", e),
                    }
                })?;
                if digest != cg.full_digest {
                    return Err(ExecutorError::TargetContentMismatch {
                        path: item.path.clone(),
                        expected: cg.full_digest.clone(),
                        actual: digest,
                    });
                }
            }
        }

        let transaction_id = format!("tx-{}", Utc::now().timestamp_millis());
        let mut successful = Vec::new();
        let mut skipped = Vec::new();
        let mut failed = Vec::new();
        let mut bytes_moved_to_trash = 0u64;
        let mut estimated_eventual_reclaim = 0u64;
        let immediate_reclaimed_bytes = 0u64;

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

            // TOCTOU Gate 2: Live filesystem inspection
            let meta = match std::fs::symlink_metadata(&item.path) {
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
                    let err_msg = format!("Failed to read target metadata: {}", e);
                    failed.push(FailedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        error: err_msg,
                    });
                    continue;
                }
            };

            // Symlinks are refused for destructive deletion
            if meta.file_type().is_symlink() {
                let reason = "Symlink substitution detected: target is a symbolic link".to_string();
                skipped.push(SkippedItem {
                    candidate_id: item.candidate_id.clone(),
                    path: item.path.clone(),
                    reason,
                });
                continue;
            }

            let (file_opt, live_ident) = if meta.is_dir() {
                if item.category == CandidateCategory::Duplicate {
                    let reason = "Duplicate candidate cannot be a directory".to_string();
                    skipped.push(SkippedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        reason,
                    });
                    continue;
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    (
                        None,
                        FileIdentity {
                            inode: meta.ino(),
                            device_id: meta.dev(),
                            size: meta.len(),
                            mtime_sec: meta.mtime(),
                            mtime_nsec: meta.mtime_nsec(),
                            ctime_sec: meta.ctime(),
                            ctime_nsec: meta.ctime_nsec(),
                            file_kind: FileKind::Directory,
                        },
                    )
                }
                #[cfg(not(unix))]
                {
                    (
                        None,
                        FileIdentity {
                            inode: 0,
                            device_id: 0,
                            size: meta.len(),
                            mtime_sec: 0,
                            mtime_nsec: 0,
                            ctime_sec: 0,
                            ctime_nsec: 0,
                            file_kind: FileKind::Directory,
                        },
                    )
                }
            } else {
                let file = match open_regular_file_safely(&item.path) {
                    Ok(f) => f,
                    Err(e) => {
                        let err_msg = format!("Failed to safely open regular file: {}", e);
                        failed.push(FailedItem {
                            candidate_id: item.candidate_id.clone(),
                            path: item.path.clone(),
                            error: err_msg,
                        });
                        continue;
                    }
                };
                let id = match query_file_identity(&file) {
                    Ok(id) => id,
                    Err(e) => {
                        let err_msg = format!("Failed to read file identity: {}", e);
                        failed.push(FailedItem {
                            candidate_id: item.candidate_id.clone(),
                            path: item.path.clone(),
                            error: err_msg,
                        });
                        continue;
                    }
                };
                if id.file_kind != FileKind::Regular {
                    let reason = "Target is not a regular file".to_string();
                    skipped.push(SkippedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        reason,
                    });
                    continue;
                }
                (Some(file), id)
            };

            // Device ID check
            if live_ident.device_id != item.expected_device_id {
                let reason = format!(
                    "TOCTOU device ID mismatch (expected {}, live {})",
                    item.expected_device_id, live_ident.device_id
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
                        Some(live_ident.inode),
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
            if live_ident.inode != item.expected_inode {
                let reason = format!(
                    "TOCTOU inode mismatch: file was replaced or recreated (expected {}, live {})",
                    item.expected_inode, live_ident.inode
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
                        Some(live_ident.inode),
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

            // Size check (for regular files)
            if live_ident.file_kind == FileKind::Regular
                && item.expected_size > 0
                && live_ident.size != item.expected_size
            {
                let reason = format!(
                    "TOCTOU size mismatch (expected {} bytes, live {} bytes)",
                    item.expected_size, live_ident.size
                );
                skipped.push(SkippedItem {
                    candidate_id: item.candidate_id.clone(),
                    path: item.path.clone(),
                    reason,
                });
                continue;
            }

            // Modification time check (seconds and nanoseconds)
            if live_ident.mtime_sec != item.expected_mtime_sec
                || live_ident.mtime_nsec != item.expected_mtime_nsec
            {
                let reason = format!(
                    "TOCTOU mtime mismatch: target modified after plan compilation (expected {}.{}, live {}.{})",
                    item.expected_mtime_sec,
                    item.expected_mtime_nsec,
                    live_ident.mtime_sec,
                    live_ident.mtime_nsec
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
                        Some(live_ident.inode),
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

            // Change time check (seconds and nanoseconds)
            if live_ident.ctime_sec != item.expected_ctime_sec
                || live_ident.ctime_nsec != item.expected_ctime_nsec
            {
                let reason = format!(
                    "TOCTOU ctime mismatch: target status changed after plan compilation (expected {}.{}, live {}.{})",
                    item.expected_ctime_sec,
                    item.expected_ctime_nsec,
                    live_ident.ctime_sec,
                    live_ident.ctime_nsec
                );
                skipped.push(SkippedItem {
                    candidate_id: item.candidate_id.clone(),
                    path: item.path.clone(),
                    reason,
                });
                continue;
            }

            // Re-validate content digest if content_guard is present
            if let Some(ref cg) = item.content_guard {
                if let Some(ref file) = file_opt {
                    match compute_file_blake3(file) {
                        Ok(digest) => {
                            if digest != cg.full_digest {
                                let reason = format!(
                                    "TOCTOU content digest mismatch: target content modified (expected {}, live {})",
                                    cg.full_digest, digest
                                );
                                skipped.push(SkippedItem {
                                    candidate_id: item.candidate_id.clone(),
                                    path: item.path.clone(),
                                    reason,
                                });
                                continue;
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to compute content digest: {}", e);
                            failed.push(FailedItem {
                                candidate_id: item.candidate_id.clone(),
                                path: item.path.clone(),
                                error: err_msg,
                            });
                            continue;
                        }
                    }
                } else {
                    let reason = "Target cannot be verified with content guard because it is not a regular file".to_string();
                    skipped.push(SkippedItem {
                        candidate_id: item.candidate_id.clone(),
                        path: item.path.clone(),
                        reason,
                    });
                    continue;
                }
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
                            Some(live_ident.inode),
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
                        Some(live_ident.inode),
                        "dry_run",
                        item.allocated_bytes,
                        true,
                        None,
                    )?;
                }
                bytes_moved_to_trash += item.allocated_bytes;
                estimated_eventual_reclaim += item.allocated_bytes;
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

            // Drop open file handle before moving to Trash
            drop(file_opt);

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
                    Some(live_ident.inode),
                    "pending",
                    item.allocated_bytes,
                    true,
                    None,
                )?;
            }

            // Action: move to Trash via backend
            match self.backend.trash(&item.path) {
                Ok(dest) => {
                    bytes_moved_to_trash += item.allocated_bytes;
                    estimated_eventual_reclaim += item.allocated_bytes;
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
                            Some(live_ident.inode),
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
                            Some(live_ident.inode),
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
            bytes_moved_to_trash,
            estimated_eventual_reclaim_after_purge: estimated_eventual_reclaim,
            immediate_reclaimed_bytes,
            total_reclaimed_bytes: bytes_moved_to_trash,
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
