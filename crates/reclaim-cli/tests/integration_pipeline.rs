use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

use reclaim_core::allocation::AllocationInfo;
use reclaim_core::candidate::CandidateCategory;
use reclaim_core::risk::RiskLevel;
use reclaim_plan::CleanupPlan;
use reclaim_risk::CandidateEvaluator;
use reclaim_rules::engine::RulesEngine;
use reclaim_scan::{FilesystemScanner, ScanOptions};

#[test]
fn test_end_to_end_pipeline_with_safety_guarantees() {
    let sandbox = tempdir().unwrap();
    let root = sandbox.path();

    // 1. Setup synthetic directories
    let xcode_dir = root.join("Library/Developer/Xcode/DerivedData/SampleApp-12345");
    fs::create_dir_all(&xcode_dir).unwrap();
    let mut f1 = File::create(xcode_dir.join("build.db")).unwrap();
    f1.write_all(&vec![0xAA; 8192]).unwrap();

    let ssh_dir = root.join(".ssh");
    fs::create_dir_all(&ssh_dir).unwrap();
    let mut f2 = File::create(ssh_dir.join("id_ed25519")).unwrap();
    f2.write_all(b"SECRET_KEY_MATERIAL").unwrap();

    let unknown_file = root.join("unknown_blob.xyz");
    let mut f3 = File::create(&unknown_file).unwrap();
    f3.write_all(b"random bytes").unwrap();

    // 2. Scan sandbox
    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: None,
    });
    let report = scanner.scan(root).unwrap();
    assert!(report.total_files >= 3);

    // 3. Evaluate candidates
    let mut engine = RulesEngine::new();
    let mut evaluator = CandidateEvaluator::new(&mut engine);

    let mut evaluated = Vec::new();
    for entry in report.entries {
        let alloc = AllocationInfo::new(entry.logical_bytes, entry.allocated_bytes, false);
        let cand = evaluator.evaluate(
            &entry.path,
            alloc,
            entry.inode,
            entry.device_id,
            entry.mtime_sec,
            entry.is_dir,
        );
        evaluated.push(cand);
    }

    // 4. Assert safety classifications
    let ssh_cand = evaluated
        .iter()
        .find(|c| c.path.ends_with(".ssh/id_ed25519"))
        .expect("SSH key must be found");
    assert_eq!(ssh_cand.risk, RiskLevel::Protected);
    assert!(!ssh_cand.is_auto_cleanable());

    let unknown_cand = evaluated
        .iter()
        .find(|c| c.path.ends_with("unknown_blob.xyz"))
        .expect("Unknown blob must be found");
    assert_eq!(unknown_cand.category, CandidateCategory::Unknown);
    assert_eq!(unknown_cand.risk, RiskLevel::Unknown);
    assert!(!unknown_cand.is_auto_cleanable());

    // 5. Build Cleanup Plan with SAFE filter
    let plan = CleanupPlan::build(&evaluated, RiskLevel::Safe, "1.0").unwrap();

    // Verify: Protected and Unknown are strictly EXCLUDED from plan
    assert!(
        !plan
            .items
            .iter()
            .any(|it| it.path.ends_with(".ssh/id_ed25519")),
        "Protected item must never be in cleanup plan"
    );
    assert!(
        !plan
            .items
            .iter()
            .any(|it| it.path.ends_with("unknown_blob.xyz")),
        "Unknown item must never be in cleanup plan"
    );

    // Verify plan integrity
    assert!(plan.verify_integrity().is_ok());

    // Verify TOCTOU pre-check on items
    for it in &plan.items {
        assert!(CleanupPlan::verify_toctou(it).is_ok());
    }
}
