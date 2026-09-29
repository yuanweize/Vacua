use crate::cache::FingerprintCache;
use crate::group::{DuplicateGroup, DuplicatePlanEstimate};
use crate::identity::ContentError;
use crate::staged::{
    run_staged_duplicate_pipeline, verify_duplicate_pair_before_deletion, DuplicateScanOptions,
};
use crate::stats::DedupStats;
use std::path::{Path, PathBuf};
use vacua_core::candidate::{Candidate, CandidateCategory};
use vacua_core::evidence::{Evidence, EvidenceSource};
use vacua_core::risk::{RecommendationValue, RiskLevel};
use vacua_index::IndexDatabase;
use vacua_plan::{CleanupPlan, ContentGuard, PreservationGuard};
use vacua_risk::CandidateEvaluator;
use vacua_rules::engine::RulesEngine;
use vacua_scan::{FilesystemScanner, ScanOptions, ScannedEntry};

pub struct DuplicateEngine;

impl DuplicateEngine {
    /// Scans a root path for exact duplicate files using the 6-stage pipeline.
    pub fn scan_path(
        root: &Path,
        options: &DuplicateScanOptions,
        cache_db: Option<&IndexDatabase>,
    ) -> Result<(Vec<DuplicateGroup>, DedupStats), ContentError> {
        let scanner = FilesystemScanner::new(ScanOptions {
            jobs: Some(options.hash_jobs),
            ..Default::default()
        });
        let report = scanner.scan(root).map_err(ContentError::Io)?;
        let entries = report.entries;

        let mut rules = RulesEngine::new();
        let mut evaluator = CandidateEvaluator::new(&mut rules);

        let cache = cache_db.map(FingerprintCache::new);

        let start = std::time::Instant::now();
        let (groups, mut stats) =
            run_staged_duplicate_pipeline(&entries, options, cache.as_ref(), |entry| {
                let cand = evaluator.evaluate(
                    &entry.path,
                    entry.to_allocation(),
                    entry.inode,
                    entry.device_id,
                    entry.mtime_sec,
                    entry.is_dir,
                );
                (cand.risk, cand.category.to_string())
            })?;
        stats.elapsed_ms = start.elapsed().as_millis() as u64;

        Ok((groups, stats))
    }

    /// Scans pre-collected ScannedEntry records directly (e.g. from index or parallel scanner).
    pub fn scan_entries<F>(
        entries: &[ScannedEntry],
        options: &DuplicateScanOptions,
        cache: Option<&FingerprintCache>,
        risk_classifier: F,
    ) -> Result<(Vec<DuplicateGroup>, DedupStats), ContentError>
    where
        F: FnMut(&ScannedEntry) -> (RiskLevel, String),
    {
        let start = std::time::Instant::now();
        let (groups, mut stats) =
            run_staged_duplicate_pipeline(entries, options, cache, risk_classifier)?;
        stats.elapsed_ms = start.elapsed().as_millis() as u64;
        Ok((groups, stats))
    }

    /// Generates an immutable, verified schema v2 CleanupPlan to remove redundant duplicate members
    /// while strictly protecting `keep_path` with a PreservationGuard.
    pub fn build_cleanup_plan(
        group: &DuplicateGroup,
        keep_path: &Path,
    ) -> Result<CleanupPlan, ContentError> {
        let keep_member = group.find_member(keep_path).ok_or_else(|| {
            ContentError::Group(format!(
                "Specified keep path '{}' is not a member of duplicate group '{}'",
                keep_path.display(),
                group.group_id
            ))
        })?;

        let mut candidates = Vec::new();

        for member in &group.members {
            if member.path == keep_member.path {
                continue;
            }

            // Stage 6: Destructive pair confirmation before inclusion in plan
            let is_match = verify_duplicate_pair_before_deletion(&keep_member.path, &member.path)?;
            if !is_match {
                return Err(ContentError::Group(format!(
                    "Destructive confirmation failed: '{}' does not match keep candidate '{}'",
                    member.path.display(),
                    keep_member.path.display()
                )));
            }

            let cand_id = format!(
                "cand-dup-{}",
                blake3::hash(member.path.to_string_lossy().as_bytes())
                    .to_hex()
                    .get(..12)
                    .unwrap_or("000000000000")
            );

            // Reconstruct candidate model with complete timestamps
            let candidate = Candidate {
                id: cand_id,
                path: member.path.clone(),
                category: CandidateCategory::Duplicate,
                allocation: member.allocation,
                risk: member.risk,
                value: RecommendationValue::Medium,
                confidence_score: 1.0,
                evidence: vec![Evidence::new(
                    EvidenceSource::FilesystemMetadata,
                    "Exact BLAKE3 content duplicate",
                    1.0,
                    format!(
                        "Verified identical to preserved file '{}'",
                        keep_member.path.display()
                    ),
                )],
                reconstructable: true,
                rebuild_consequence: Some(format!(
                    "Redundant duplicate copy removed; canonical original preserved at '{}'",
                    keep_member.path.display()
                )),
                inode: member.inode,
                device_id: member.device_id,
                mtime_sec: member.mtime_sec,
                mtime_nsec: member.mtime_nsec,
                ctime_sec: member.ctime_sec,
                ctime_nsec: member.ctime_nsec,
            };

            candidates.push(candidate);
        }

        // Build cleanup plan using vacua_plan
        let mut plan = CleanupPlan::build(&candidates, RiskLevel::Review, "vacua-dedup-v2")
            .map_err(|e| ContentError::Group(e.to_string()))?;

        // 1. Create PreservationGuard for the kept file
        let keep_guard = PreservationGuard {
            path: keep_member.path.clone(),
            expected_device_id: keep_member.device_id,
            expected_inode: keep_member.inode,
            expected_size: group.logical_size,
            expected_mtime_sec: keep_member.mtime_sec,
            expected_mtime_nsec: keep_member.mtime_nsec,
            expected_ctime_sec: keep_member.ctime_sec,
            expected_ctime_nsec: keep_member.ctime_nsec,
            content_algorithm: "BLAKE3".to_string(),
            full_digest: group.full_digest.clone(),
        };
        plan.preservation_guards.push(keep_guard);

        // 2. Attach ContentGuard to all duplicate deletion items
        for item in &mut plan.items {
            item.content_guard = Some(ContentGuard {
                algorithm: "BLAKE3".to_string(),
                full_digest: group.full_digest.clone(),
            });
        }

        // 3. Dynamically calculate keep-dependent reclaim estimates
        let remove_paths: Vec<PathBuf> = plan.items.iter().map(|i| i.path.clone()).collect();
        let reclaim_estimate = DuplicatePlanEstimate::for_keep(group, keep_path, &remove_paths);
        plan.estimated_eventual_reclaim_bytes = reclaim_estimate.estimated_reclaimable_bytes;

        // 4. Recompute authoritative plan hash v2 covering all guards & items
        plan.recompute_hash();

        Ok(plan)
    }
}
