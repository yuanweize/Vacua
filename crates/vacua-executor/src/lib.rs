pub mod backend;
pub mod executor;
pub mod journal;

pub use backend::{MacOSTrashBackend, TempTrashBackend, TrashBackend};
pub use executor::{
    ExecutedItem, ExecutionReport, ExecutorError, FailedItem, PlanExecutor, SkippedItem,
};
pub use journal::{
    ExecutionJournal, JournalError, JournalRecord, TransactionSummary, VerificationReport,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::tempdir;
    use vacua_core::allocation::AllocationInfo;
    use vacua_core::candidate::{Candidate, CandidateCategory};
    use vacua_core::evidence::{Evidence, EvidenceSource};
    use vacua_core::risk::{RecommendationValue, RiskLevel};
    use vacua_plan::{CleanupPlan, ContentGuard, PreservationGuard};

    fn make_test_candidate(path: PathBuf, risk: RiskLevel) -> Candidate {
        let meta = fs::symlink_metadata(&path).unwrap();
        Candidate {
            id: format!("cand-{}", path.file_name().unwrap().to_string_lossy()),
            path,
            category: CandidateCategory::BuildArtifact,
            allocation: AllocationInfo::new(meta.len(), meta.blocks() * 512, false),
            risk,
            value: RecommendationValue::High,
            confidence_score: 1.0,
            evidence: vec![Evidence::new(
                EvidenceSource::PathSemantic,
                "test_signal",
                1.0,
                "synthetic test evidence",
            )],
            reconstructable: true,
            rebuild_consequence: Some("Rebuild on demand".to_string()),
            device_id: meta.dev(),
            inode: meta.ino(),
            mtime_sec: meta.mtime(),
            mtime_nsec: meta.mtime_nsec(),
            ctime_sec: meta.ctime(),
            ctime_nsec: meta.ctime_nsec(),
        }
    }

    #[test]
    fn test_executor_dry_run_preserves_filesystem() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("cache.tmp");
        fs::write(&file_path, b"ephemeral cache content").unwrap();

        let cand = make_test_candidate(file_path.clone(), RiskLevel::Safe);
        let plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "v1.0").unwrap();

        let trash_dir = dir.path().join("test_trash");
        let backend = TempTrashBackend::new(trash_dir);
        let executor = PlanExecutor::new(&backend, true); // dry run = true

        let journal_path = dir.path().join("journal.db");
        let mut journal = ExecutionJournal::open(&journal_path).unwrap();

        let report = executor.execute(&plan, Some(&mut journal)).unwrap();
        assert_eq!(report.successful_items.len(), 1);
        assert_eq!(report.skipped_items.len(), 0);
        assert_eq!(report.failed_items.len(), 0);
        assert_eq!(report.immediate_reclaimed_bytes, 0);
        assert!(report.bytes_moved_to_trash > 0);

        // Crucial invariant: file still exists untouched on disk after dry-run
        assert!(file_path.exists());
    }

    #[test]
    fn test_executor_trash_execution_and_journaling() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("build_artifact.o");
        fs::write(&file_path, b"compiled object").unwrap();

        let cand = make_test_candidate(file_path.clone(), RiskLevel::Safe);
        let plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "v1.0").unwrap();

        let trash_dir = dir.path().join("test_trash");
        let backend = TempTrashBackend::new(trash_dir.clone());
        let executor = PlanExecutor::new(&backend, false); // live execution

        let journal_path = dir.path().join("journal.db");
        let mut journal = ExecutionJournal::open(&journal_path).unwrap();

        let report = executor.execute(&plan, Some(&mut journal)).unwrap();
        assert_eq!(report.successful_items.len(), 1);
        assert_eq!(report.skipped_items.len(), 0);
        assert!(report.bytes_moved_to_trash > 0);
        assert_eq!(report.immediate_reclaimed_bytes, 0); // Native trash does not immediately free space!
        assert_eq!(
            report.estimated_eventual_reclaim_after_purge,
            report.bytes_moved_to_trash
        );

        // Original file moved out of original location
        assert!(!file_path.exists());

        // File is now inside trash directory
        assert!(trash_dir.join("build_artifact.o").exists());

        // Verify journal recorded the transaction
        let txs = journal.list_transactions(10).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].transaction_id, report.transaction_id);
        assert_eq!(txs[0].successful_count, 1);

        let records = journal
            .get_transaction_details(&report.transaction_id)
            .unwrap();
        // 1 pre-action intent + 1 success record = 2 records
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].action, "trash_intent");
        assert_eq!(records[0].result, "pending");
        assert_eq!(records[1].action, "trash");
        assert_eq!(records[1].result, "success");
        assert!(records[1].reversible);

        // Verify cryptographic hash chain on journal
        let verify_report = journal.verify_chain().unwrap();
        assert!(verify_report.is_valid);
        assert_eq!(verify_report.total_records, 2);
    }

    #[test]
    fn test_executor_toctou_inode_mismatch_skips() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("ephemeral.log");
        fs::write(&file_path, b"initial log").unwrap();

        // 1. Build plan with initial file
        let cand = make_test_candidate(file_path.clone(), RiskLevel::Safe);
        let plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "v1.0").unwrap();

        // 2. Recreate / replace file to change its inode
        fs::remove_file(&file_path).unwrap();
        fs::write(&file_path, b"recreated file with new inode").unwrap();

        let trash_dir = dir.path().join("test_trash");
        let backend = TempTrashBackend::new(trash_dir);
        let executor = PlanExecutor::new(&backend, false);

        let journal_path = dir.path().join("journal.db");
        let mut journal = ExecutionJournal::open(&journal_path).unwrap();

        let report = executor.execute(&plan, Some(&mut journal)).unwrap();

        // TOCTOU verification must detect inode change and SKIP
        assert_eq!(report.successful_items.len(), 0);
        assert_eq!(report.skipped_items.len(), 1);
        assert!(report.skipped_items[0]
            .reason
            .contains("TOCTOU inode mismatch"));

        // File must NOT be deleted
        assert!(file_path.exists());
    }

    #[test]
    fn test_executor_toctou_mtime_mismatch_skips() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("state.cache");
        fs::write(&file_path, b"version 1").unwrap();

        // 1. Build plan
        let cand = make_test_candidate(file_path.clone(), RiskLevel::Safe);
        let plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "v1.0").unwrap();

        // 2. Sleep to ensure mtime advances across filesystem second/millisecond boundary and change content with same byte length
        std::thread::sleep(Duration::from_millis(1100));
        fs::write(&file_path, b"version 2").unwrap();

        let trash_dir = dir.path().join("test_trash");
        let backend = TempTrashBackend::new(trash_dir);
        let executor = PlanExecutor::new(&backend, false);

        let journal_path = dir.path().join("journal.db");
        let mut journal = ExecutionJournal::open(&journal_path).unwrap();

        let report = executor.execute(&plan, Some(&mut journal)).unwrap();

        // Must skip due to mtime modification
        assert_eq!(report.successful_items.len(), 0);
        assert_eq!(report.skipped_items.len(), 1);
        assert!(report.skipped_items[0]
            .reason
            .contains("TOCTOU mtime mismatch"));
        assert!(file_path.exists());
    }

    #[test]
    fn test_executor_tampered_plan_fails_immediately() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("safe.tmp");
        fs::write(&file_path, b"data").unwrap();

        let cand = make_test_candidate(file_path, RiskLevel::Safe);
        let mut plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "v1.0").unwrap();

        // Tamper with plan hash
        plan.plan_hash = "tampered_hash_value".to_string();

        let trash_dir = dir.path().join("test_trash");
        let backend = TempTrashBackend::new(trash_dir);
        let executor = PlanExecutor::new(&backend, false);

        let err = executor.execute(&plan, None).unwrap_err();
        match err {
            ExecutorError::PlanHashMismatch => (),
            other => panic!("Expected PlanHashMismatch, got: {:?}", other),
        }
    }

    #[test]
    fn test_executor_legacy_v1_plan_refused() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("safe.tmp");
        fs::write(&file_path, b"data").unwrap();

        let cand = make_test_candidate(file_path, RiskLevel::Safe);
        let mut plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "v1.0").unwrap();

        // Simulate legacy v1 plan
        plan.plan_schema_version = 1;
        // Recompute hash so hash matches legacy schema
        plan.plan_hash = plan.compute_hash();

        let trash_dir = dir.path().join("test_trash");
        let backend = TempTrashBackend::new(trash_dir.clone());

        // Dry-run inspection should be allowed
        let dry_executor = PlanExecutor::new(&backend, true);
        assert!(dry_executor.execute(&plan, None).is_ok());

        // Destructive execution MUST BE REFUSED
        let live_executor = PlanExecutor::new(&backend, false);
        let err = live_executor.execute(&plan, None).unwrap_err();
        match err {
            ExecutorError::PlanExecutionRefused(msg) => {
                assert!(
                    msg.contains("Legacy plan schema is not eligible for destructive execution")
                );
            }
            other => panic!("Expected PlanExecutionRefused, got: {:?}", other),
        }
    }

    #[test]
    fn test_executor_preservation_guard_failure_aborts_all() {
        let dir = tempdir().unwrap();
        let keep_file = dir.path().join("original.txt");
        let dup_file = dir.path().join("duplicate.txt");
        fs::write(&keep_file, b"identical content").unwrap();
        fs::write(&dup_file, b"identical content").unwrap();

        let keep_meta = fs::symlink_metadata(&keep_file).unwrap();
        let keep_digest = blake3::hash(b"identical content").to_hex().to_string();

        let dup_cand = make_test_candidate(dup_file.clone(), RiskLevel::Safe);
        let mut plan = CleanupPlan::build(&[dup_cand], RiskLevel::Safe, "v1.0").unwrap();

        // Attach PreservationGuard for keep_file
        let guard = PreservationGuard {
            path: keep_file.clone(),
            expected_device_id: keep_meta.dev(),
            expected_inode: keep_meta.ino(),
            expected_size: keep_meta.len(),
            expected_mtime_sec: keep_meta.mtime(),
            expected_mtime_nsec: keep_meta.mtime_nsec(),
            expected_ctime_sec: keep_meta.ctime(),
            expected_ctime_nsec: keep_meta.ctime_nsec(),
            content_algorithm: "BLAKE3".to_string(),
            full_digest: keep_digest,
        };
        plan.preservation_guards.push(guard);
        plan.plan_hash = plan.compute_hash();

        // Mutate keep_file BEFORE execution!
        fs::write(&keep_file, b"MUTATED content!").unwrap();

        let trash_dir = dir.path().join("test_trash");
        let backend = TempTrashBackend::new(trash_dir);
        let executor = PlanExecutor::new(&backend, false);

        // Preflight MUST fail and dup_file MUST NOT be deleted
        let err = executor.execute(&plan, None).unwrap_err();
        match err {
            ExecutorError::PreservationGuardFailed { path, reason } => {
                assert_eq!(path, keep_file);
                assert!(reason.contains("mismatch") || reason.contains("modified"));
            }
            other => panic!("Expected PreservationGuardFailed, got: {:?}", other),
        }

        // duplicate file must remain completely untouched on disk
        assert!(dup_file.exists());
    }

    #[test]
    fn test_executor_target_content_guard_mismatch_aborts() {
        let dir = tempdir().unwrap();
        let dup_file = dir.path().join("target.txt");
        fs::write(&dup_file, b"original target content").unwrap();

        let dup_cand = make_test_candidate(dup_file.clone(), RiskLevel::Safe);
        let mut plan = CleanupPlan::build(&[dup_cand], RiskLevel::Safe, "v1.0").unwrap();

        // Attach ContentGuard for dup_file
        let digest = blake3::hash(b"original target content")
            .to_hex()
            .to_string();
        plan.items[0].content_guard = Some(ContentGuard {
            algorithm: "BLAKE3".to_string(),
            full_digest: digest,
        });
        plan.plan_hash = plan.compute_hash();

        // Mutate dup_file in-place before execution
        fs::write(&dup_file, b"modified target content!").unwrap();

        let trash_dir = dir.path().join("test_trash");
        let backend = TempTrashBackend::new(trash_dir);
        let executor = PlanExecutor::new(&backend, false);

        let err = executor.execute(&plan, None).unwrap_err();
        match err {
            ExecutorError::TargetContentMismatch { path, expected, .. } => {
                assert_eq!(path, dup_file);
                assert_eq!(
                    expected,
                    blake3::hash(b"original target content")
                        .to_hex()
                        .to_string()
                );
            }
            other => panic!("Expected TargetContentMismatch, got: {:?}", other),
        }

        // Target file must NOT be moved to trash
        assert!(dup_file.exists());
    }

    #[test]
    fn test_journal_tamper_detection() {
        let dir = tempdir().unwrap();
        let journal_path = dir.path().join("audit.db");
        let mut journal = ExecutionJournal::open(&journal_path).unwrap();

        // Write 3 legitimate entries
        journal
            .record(
                "tx-1",
                "hash-1",
                "cand-1",
                "/tmp/file1",
                "trash",
                "SAFE",
                "rule1",
                None,
                None,
                "success",
                100,
                true,
                None,
            )
            .unwrap();
        journal
            .record(
                "tx-1",
                "hash-1",
                "cand-2",
                "/tmp/file2",
                "trash",
                "SAFE",
                "rule1",
                None,
                None,
                "success",
                200,
                true,
                None,
            )
            .unwrap();
        journal
            .record(
                "tx-2",
                "hash-2",
                "cand-3",
                "/tmp/file3",
                "trash",
                "SAFE",
                "rule2",
                None,
                None,
                "success",
                300,
                true,
                None,
            )
            .unwrap();

        // 1. Initial verification should succeed
        let report = journal.verify_chain().unwrap();
        assert!(report.is_valid);
        assert_eq!(report.total_records, 3);
        assert!(report.broken_record_id.is_none());

        // 2. Deliberately tamper with record 2 via raw SQL (simulate attacker editing DB)
        let conn = rusqlite::Connection::open(&journal_path).unwrap();
        conn.execute(
            "UPDATE execution_journal SET path = '/tmp/tampered_path' WHERE id = 2",
            [],
        )
        .unwrap();

        // 3. Chain verification must catch the tampering at record 2
        let tampered_report = journal.verify_chain().unwrap();
        assert!(!tampered_report.is_valid);
        assert_eq!(tampered_report.broken_record_id, Some(2));
        assert!(tampered_report
            .error_detail
            .unwrap()
            .contains("Hash mismatch at record ID 2"));
    }
}
