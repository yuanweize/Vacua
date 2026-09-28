pub mod backend;
pub mod executor;
pub mod journal;

pub use backend::{MacOSTrashBackend, TempTrashBackend, TrashBackend};
pub use executor::{
    ExecutedItem, ExecutionReport, ExecutorError, FailedItem, PlanExecutor, SkippedItem,
};
pub use journal::{ExecutionJournal, JournalError, JournalRecord, TransactionSummary};

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
    use vacua_plan::CleanupPlan;

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
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].result, "success");
        assert!(records[0].reversible);
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

        // 2. Sleep to ensure mtime advances across filesystem second/millisecond boundary and append content
        std::thread::sleep(Duration::from_millis(1100));
        fs::write(&file_path, b"version 2 modified content").unwrap();

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
}
