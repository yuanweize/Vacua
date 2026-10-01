use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use vacua_core::candidate::{Candidate, CandidateCategory};
use vacua_core::error::{ReclaimError, Result};
use vacua_core::fs::{open_regular_file_safely, query_file_identity, FileKind};
use vacua_core::risk::RiskLevel;

pub const CURRENT_PLAN_SCHEMA_VERSION: u32 = 2;
pub const PLAN_DOMAIN_SEPARATOR_V2: &[u8] = b"VACUA_PLAN_V2\n";

fn default_schema_version() -> u32 {
    1
}

/// Cryptographic content digest guard ensuring target content matches expected hash before deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentGuard {
    pub algorithm: String, // e.g. "BLAKE3"
    pub full_digest: String,
}

/// Cryptographic preservation guard recording the user-chosen keep copy.
/// Execution MUST verify that the preserved copy still exists, is a regular file,
/// has matching filesystem identity, and matches content digest before any mutation occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreservationGuard {
    pub path: PathBuf,
    pub expected_device_id: u64,
    pub expected_inode: u64,
    pub expected_size: u64,
    pub expected_mtime_sec: i64,
    pub expected_mtime_nsec: i64,
    pub expected_ctime_sec: i64,
    pub expected_ctime_nsec: i64,
    pub content_algorithm: String,
    pub full_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanItem {
    pub candidate_id: String,
    pub path: PathBuf,
    pub category: CandidateCategory,
    pub risk: RiskLevel,
    pub allocated_bytes: u64,
    pub expected_inode: u64,
    pub expected_device_id: u64,
    #[serde(default)]
    pub expected_size: u64,
    pub expected_mtime_sec: i64,
    #[serde(default)]
    pub expected_mtime_nsec: i64,
    #[serde(default)]
    pub expected_ctime_sec: i64,
    #[serde(default)]
    pub expected_ctime_nsec: i64,
    #[serde(default)]
    pub content_guard: Option<ContentGuard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanRiskSummary {
    pub safe_count: usize,
    pub review_count: usize,
    pub caution_count: usize,
    pub safe_allocated_bytes: u64,
    pub review_allocated_bytes: u64,
    pub caution_allocated_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPlan {
    #[serde(default = "default_schema_version")]
    pub plan_schema_version: u32,
    pub plan_id: String,
    pub created_at: DateTime<Utc>,
    pub ruleset_version: String,
    pub items: Vec<PlanItem>,
    #[serde(default)]
    pub preservation_guards: Vec<PreservationGuard>,
    #[serde(alias = "estimated_physical_reclaim")]
    pub estimated_eventual_reclaim_bytes: u64,
    pub risk_summary: PlanRiskSummary,
    pub plan_hash: String,
}

impl CleanupPlan {
    /// Builds a standard schema v2 CleanupPlan from candidate items.
    pub fn build(
        candidates: &[Candidate],
        max_allowed_risk: RiskLevel,
        ruleset_version: &str,
    ) -> Result<Self> {
        Self::build_with_guards(candidates, max_allowed_risk, ruleset_version, Vec::new())
    }

    /// Builds a schema v2 CleanupPlan with optional preservation guards (used for duplicate cleanup).
    pub fn build_with_guards(
        candidates: &[Candidate],
        max_allowed_risk: RiskLevel,
        ruleset_version: &str,
        preservation_guards: Vec<PreservationGuard>,
    ) -> Result<Self> {
        let mut items = Vec::new();
        let mut risk_counts: HashMap<RiskLevel, (usize, u64)> = HashMap::new();
        let mut total_reclaim = 0u64;

        for cand in candidates {
            // Hard safety rule: Never include Protected or Unknown
            if cand.risk.is_protected_or_unknown() {
                continue;
            }

            // Must satisfy max allowed risk threshold
            if cand.risk > max_allowed_risk {
                continue;
            }

            let entry = risk_counts.entry(cand.risk).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry.1.saturating_add(cand.allocation.allocated_bytes);
            total_reclaim =
                total_reclaim.saturating_add(cand.allocation.potentially_reclaimable_bytes);

            items.push(PlanItem {
                candidate_id: cand.id.clone(),
                path: cand.path.clone(),
                category: cand.category,
                risk: cand.risk,
                allocated_bytes: cand.allocation.allocated_bytes,
                expected_inode: cand.inode,
                expected_device_id: cand.device_id,
                expected_size: cand.allocation.logical_bytes,
                expected_mtime_sec: cand.mtime_sec,
                expected_mtime_nsec: cand.mtime_nsec,
                expected_ctime_sec: cand.ctime_sec,
                expected_ctime_nsec: cand.ctime_nsec,
                content_guard: None,
            });
        }

        let (safe_count, safe_allocated_bytes) =
            risk_counts.get(&RiskLevel::Safe).copied().unwrap_or((0, 0));
        let (review_count, review_allocated_bytes) = risk_counts
            .get(&RiskLevel::Review)
            .copied()
            .unwrap_or((0, 0));
        let (caution_count, caution_allocated_bytes) = risk_counts
            .get(&RiskLevel::Caution)
            .copied()
            .unwrap_or((0, 0));

        let risk_summary = PlanRiskSummary {
            safe_count,
            review_count,
            caution_count,
            safe_allocated_bytes,
            review_allocated_bytes,
            caution_allocated_bytes,
        };

        let now = Utc::now();
        let plan_hash = compute_plan_hash_v2(
            CURRENT_PLAN_SCHEMA_VERSION,
            ruleset_version,
            &now,
            total_reclaim,
            &items,
            &preservation_guards,
        );
        let plan_id = format!("plan-{}", &plan_hash[..12]);

        Ok(Self {
            plan_schema_version: CURRENT_PLAN_SCHEMA_VERSION,
            plan_id,
            created_at: now,
            ruleset_version: ruleset_version.to_string(),
            items,
            preservation_guards,
            estimated_eventual_reclaim_bytes: total_reclaim,
            risk_summary,
            plan_hash,
        })
    }

    /// Computes the authoritative hash for the plan.
    pub fn compute_hash(&self) -> String {
        if self.plan_schema_version >= 2 {
            compute_plan_hash_v2(
                self.plan_schema_version,
                &self.ruleset_version,
                &self.created_at,
                self.estimated_eventual_reclaim_bytes,
                &self.items,
                &self.preservation_guards,
            )
        } else {
            compute_plan_hash_v1(&self.items, &self.created_at)
        }
    }

    /// Recomputes and updates the plan_hash field.
    pub fn recompute_hash(&mut self) {
        self.plan_hash = self.compute_hash();
    }

    /// Re-verify plan integrity against its embedded SHA-256 hash.
    pub fn verify_integrity(&self) -> Result<()> {
        let computed = self.compute_hash();

        if computed != self.plan_hash {
            return Err(ReclaimError::PlanIntegrityFailure {
                expected: self.plan_hash.clone(),
                computed,
            });
        }
        Ok(())
    }

    /// Verifies eligibility for destructive execution.
    /// Legacy schema v1 plans are REFUSED for destructive execution.
    pub fn verify_for_destructive_execution(&self) -> Result<()> {
        self.verify_integrity()?;

        if self.plan_schema_version < 2 {
            return Err(ReclaimError::PlanExecutionRefused {
                reason: "Legacy plan schema is not eligible for destructive execution. Regenerate the plan with Vacua >= 0.4.1.".to_string(),
            });
        }

        // Duplicate plans must have at least one preservation guard
        let has_duplicates = self
            .items
            .iter()
            .any(|i| i.category == CandidateCategory::Duplicate);
        if has_duplicates && self.preservation_guards.is_empty() {
            return Err(ReclaimError::PlanExecutionRefused {
                reason: "Duplicate cleanup plan missing required preservation guard. Regenerate with a verified keep target.".to_string(),
            });
        }

        Ok(())
    }

    /// Builds a schema v2 CleanupPlan from user-selected candidate group IDs.
    pub fn build_from_groups(
        candidates: &[Candidate],
        selected_group_ids: &[String],
        root_path: &Path,
        ruleset_version: &str,
    ) -> Result<Self> {
        let (safe_groups, _, _) = vacua_core::rescue::group_candidates(candidates, root_path);
        let selected_cand_ids: std::collections::HashSet<String> = safe_groups
            .into_iter()
            .filter(|g| selected_group_ids.contains(&g.group_id))
            .flat_map(|g| g.candidate_ids)
            .collect();

        let filtered_candidates: Vec<Candidate> = candidates
            .iter()
            .filter(|c| selected_cand_ids.contains(&c.id))
            .cloned()
            .collect();

        Self::build(&filtered_candidates, RiskLevel::Safe, ruleset_version)
    }

    /// Verifies preservation guard integrity before plan execution.
    pub fn verify_preservation_guard(guard: &PreservationGuard) -> Result<()> {
        let file =
            open_regular_file_safely(&guard.path).map_err(|e| ReclaimError::ToctouMismatch {
                path: guard.path.display().to_string(),
                reason: format!("Cannot safely open preserved file: {}", e),
            })?;
        let ident = query_file_identity(&file).map_err(|e| ReclaimError::ToctouMismatch {
            path: guard.path.display().to_string(),
            reason: format!("Failed to query identity of preserved file: {}", e),
        })?;
        if ident.file_kind != FileKind::Regular {
            return Err(ReclaimError::ToctouMismatch {
                path: guard.path.display().to_string(),
                reason: "Preserved path is not a regular file".to_string(),
            });
        }
        if ident.device_id != guard.expected_device_id || ident.inode != guard.expected_inode {
            return Err(ReclaimError::ToctouMismatch {
                path: guard.path.display().to_string(),
                reason: "Preserved file identity mismatch".to_string(),
            });
        }
        if ident.size != guard.expected_size || ident.mtime_sec != guard.expected_mtime_sec {
            return Err(ReclaimError::ToctouMismatch {
                path: guard.path.display().to_string(),
                reason: "Preserved file metadata modified".to_string(),
            });
        }
        Ok(())
    }

    /// Performs complete preflight verification of the entire plan and all targets right before execution.
    pub fn preflight_check(&self) -> Result<()> {
        self.verify_for_destructive_execution()?;
        for item in &self.items {
            Self::verify_toctou(item)?;
        }
        for guard in &self.preservation_guards {
            Self::verify_preservation_guard(guard)?;
        }
        Ok(())
    }

    /// Checks if filesystem state has changed since plan compilation.
    pub fn is_stale(&self) -> bool {
        self.preflight_check().is_err()
    }

    /// Verify an individual item against filesystem state immediately before execution (TOCTOU defense).
    /// Uses open_regular_file_safely to prevent symlink substitution and non-blocking FIFO access on files,
    /// and symlink_metadata for directory candidate verification.
    pub fn verify_toctou(item: &PlanItem) -> Result<()> {
        let meta =
            std::fs::symlink_metadata(&item.path).map_err(|e| ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!("failed to read metadata: {}", e),
            })?;

        if meta.file_type().is_symlink() {
            return Err(ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: "symlink substitution detected".to_string(),
            });
        }

        let (inode, dev, size, mtime_sec, mtime_nsec, ctime_sec, ctime_nsec) = if meta.is_dir() {
            use std::os::unix::fs::MetadataExt;
            (
                meta.ino(),
                meta.dev(),
                meta.len(),
                meta.mtime(),
                meta.mtime_nsec(),
                meta.ctime(),
                meta.ctime_nsec(),
            )
        } else {
            let file =
                open_regular_file_safely(&item.path).map_err(|e| ReclaimError::ToctouMismatch {
                    path: item.path.display().to_string(),
                    reason: format!("failed to open regular file safely: {}", e),
                })?;

            let id = query_file_identity(&file).map_err(|e| ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!("failed to query file identity: {}", e),
            })?;

            (
                id.inode,
                id.device_id,
                id.size,
                id.mtime_sec,
                id.mtime_nsec,
                id.ctime_sec,
                id.ctime_nsec,
            )
        };

        // 1. Inode match check
        if inode != item.expected_inode {
            return Err(ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!(
                    "inode mismatch (expected {}, found {})",
                    item.expected_inode, inode
                ),
            });
        }

        // 2. Device match check
        if dev != item.expected_device_id {
            return Err(ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!(
                    "device mismatch (expected {}, found {})",
                    item.expected_device_id, dev
                ),
            });
        }

        // 3. Size match check (for regular files)
        if !meta.is_dir() && item.expected_size > 0 && size != item.expected_size {
            return Err(ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!(
                    "size mismatch (expected {} bytes, found {} bytes)",
                    item.expected_size, size
                ),
            });
        }

        // 4. mtime seconds match
        if mtime_sec != item.expected_mtime_sec {
            return Err(ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!(
                    "mtime_sec mismatch (expected {}, found {})",
                    item.expected_mtime_sec, mtime_sec
                ),
            });
        }

        // 5. mtime nanoseconds match
        if item.expected_mtime_nsec > 0 && mtime_nsec != item.expected_mtime_nsec {
            return Err(ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!(
                    "mtime_nsec mismatch (expected {}, found {})",
                    item.expected_mtime_nsec, mtime_nsec
                ),
            });
        }

        // 6. ctime seconds match
        if item.expected_ctime_sec > 0 && ctime_sec != item.expected_ctime_sec {
            return Err(ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!(
                    "ctime_sec mismatch (expected {}, found {})",
                    item.expected_ctime_sec, ctime_sec
                ),
            });
        }

        // 7. ctime nanoseconds match
        if item.expected_ctime_nsec > 0 && ctime_nsec != item.expected_ctime_nsec {
            return Err(ReclaimError::ToctouMismatch {
                path: item.path.display().to_string(),
                reason: format!(
                    "ctime_nsec mismatch (expected {}, found {})",
                    item.expected_ctime_nsec, ctime_nsec
                ),
            });
        }

        Ok(())
    }
}

/// Helper function to hash raw path bytes without UTF-8 lossiness.
fn hash_raw_path(hasher: &mut Sha256, path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_os_str().as_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    #[cfg(not(unix))]
    {
        let s = path.to_string_lossy();
        let bytes = s.as_bytes();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
}

/// Schema v2 canonical deterministic plan hash.
pub fn compute_plan_hash_v2(
    schema_version: u32,
    ruleset_version: &str,
    created_at: &DateTime<Utc>,
    reclaim_bytes: u64,
    items: &[PlanItem],
    preservation_guards: &[PreservationGuard],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PLAN_DOMAIN_SEPARATOR_V2);

    hasher.update(b"SCHEMA:");
    hasher.update(schema_version.to_le_bytes());

    hasher.update(b"RULESET:");
    hasher.update((ruleset_version.len() as u64).to_le_bytes());
    hasher.update(ruleset_version.as_bytes());

    let date_str = created_at.to_rfc3339();
    hasher.update(b"CREATED:");
    hasher.update((date_str.len() as u64).to_le_bytes());
    hasher.update(date_str.as_bytes());

    hasher.update(b"RECLAIM:");
    hasher.update(reclaim_bytes.to_le_bytes());

    // Hash preservation guards canonically
    hasher.update(b"PRESERVATION_COUNT:");
    hasher.update((preservation_guards.len() as u64).to_le_bytes());
    for pg in preservation_guards {
        hasher.update(b"PG:");
        hash_raw_path(&mut hasher, &pg.path);
        hasher.update(pg.expected_device_id.to_le_bytes());
        hasher.update(pg.expected_inode.to_le_bytes());
        hasher.update(pg.expected_size.to_le_bytes());
        hasher.update(pg.expected_mtime_sec.to_le_bytes());
        hasher.update(pg.expected_mtime_nsec.to_le_bytes());
        hasher.update(pg.expected_ctime_sec.to_le_bytes());
        hasher.update(pg.expected_ctime_nsec.to_le_bytes());
        hasher.update((pg.content_algorithm.len() as u64).to_le_bytes());
        hasher.update(pg.content_algorithm.as_bytes());
        hasher.update((pg.full_digest.len() as u64).to_le_bytes());
        hasher.update(pg.full_digest.as_bytes());
    }

    // Hash plan items canonically
    hasher.update(b"ITEMS_COUNT:");
    hasher.update((items.len() as u64).to_le_bytes());
    for it in items {
        hasher.update(b"ITEM:");
        hasher.update((it.candidate_id.len() as u64).to_le_bytes());
        hasher.update(it.candidate_id.as_bytes());

        hash_raw_path(&mut hasher, &it.path);

        let cat_str = it.category.as_str();
        hasher.update((cat_str.len() as u64).to_le_bytes());
        hasher.update(cat_str.as_bytes());

        let risk_str = it.risk.as_str();
        hasher.update((risk_str.len() as u64).to_le_bytes());
        hasher.update(risk_str.as_bytes());

        hasher.update(it.allocated_bytes.to_le_bytes());
        hasher.update(it.expected_inode.to_le_bytes());
        hasher.update(it.expected_device_id.to_le_bytes());
        hasher.update(it.expected_size.to_le_bytes());
        hasher.update(it.expected_mtime_sec.to_le_bytes());
        hasher.update(it.expected_mtime_nsec.to_le_bytes());
        hasher.update(it.expected_ctime_sec.to_le_bytes());
        hasher.update(it.expected_ctime_nsec.to_le_bytes());

        if let Some(cg) = &it.content_guard {
            hasher.update(b"CG:Y");
            hasher.update((cg.algorithm.len() as u64).to_le_bytes());
            hasher.update(cg.algorithm.as_bytes());
            hasher.update((cg.full_digest.len() as u64).to_le_bytes());
            hasher.update(cg.full_digest.as_bytes());
        } else {
            hasher.update(b"CG:N");
        }
    }

    let res = hasher.finalize();
    res.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Legacy schema v1 plan hash implementation (retained exclusively for inspection integrity checks).
pub fn compute_plan_hash_v1(items: &[PlanItem], created_at: &DateTime<Utc>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(created_at.to_rfc3339().as_bytes());
    for it in items {
        hasher.update(it.candidate_id.as_bytes());
        hasher.update(it.path.to_string_lossy().as_bytes());
        hasher.update(it.allocated_bytes.to_le_bytes());
        hasher.update(it.expected_inode.to_le_bytes());
        hasher.update(it.expected_device_id.to_le_bytes());
    }
    let res = hasher.finalize();
    res.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use vacua_core::allocation::AllocationInfo;
    use vacua_core::risk::RecommendationValue;

    fn make_test_candidate(path: PathBuf, risk: RiskLevel, cat: CandidateCategory) -> Candidate {
        Candidate {
            id: format!("cand-{}", path.file_name().unwrap().to_str().unwrap()),
            path,
            category: cat,
            allocation: AllocationInfo::new(1024, 1024, false),
            risk,
            value: RecommendationValue::High,
            confidence_score: 0.9,
            evidence: vec![],
            reconstructable: true,
            rebuild_consequence: None,
            inode: 12345,
            device_id: 67890,
            mtime_sec: 1700000000,
            mtime_nsec: 500,
            ctime_sec: 1700000001,
            ctime_nsec: 600,
        }
    }

    #[test]
    fn test_plan_building_and_integrity_v2() {
        let f = NamedTempFile::new().unwrap();
        let cand = make_test_candidate(
            f.path().to_path_buf(),
            RiskLevel::Safe,
            CandidateCategory::Cache,
        );
        let plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "test-rules-v1").unwrap();

        assert_eq!(plan.plan_schema_version, 2);
        assert_eq!(plan.items.len(), 1);
        assert!(plan.verify_integrity().is_ok());
        assert!(plan.verify_for_destructive_execution().is_ok());
    }

    #[test]
    fn test_plan_hash_mutation_detected_for_all_fields() {
        let f = NamedTempFile::new().unwrap();
        let cand = make_test_candidate(
            f.path().to_path_buf(),
            RiskLevel::Safe,
            CandidateCategory::Cache,
        );
        let plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "rules-v1").unwrap();
        assert!(plan.verify_integrity().is_ok());

        // 1. Mutate risk
        let mut p = plan.clone();
        p.items[0].risk = RiskLevel::Review;
        assert!(
            p.verify_integrity().is_err(),
            "mutating risk must invalidate hash"
        );

        // 2. Mutate category
        let mut p = plan.clone();
        p.items[0].category = CandidateCategory::UserDocument;
        assert!(
            p.verify_integrity().is_err(),
            "mutating category must invalidate hash"
        );

        // 3. Mutate expected_size
        let mut p = plan.clone();
        p.items[0].expected_size += 1;
        assert!(
            p.verify_integrity().is_err(),
            "mutating expected_size must invalidate hash"
        );

        // 4. Mutate mtime_nsec
        let mut p = plan.clone();
        p.items[0].expected_mtime_nsec += 1;
        assert!(
            p.verify_integrity().is_err(),
            "mutating mtime_nsec must invalidate hash"
        );

        // 5. Mutate ctime_nsec
        let mut p = plan.clone();
        p.items[0].expected_ctime_nsec += 1;
        assert!(
            p.verify_integrity().is_err(),
            "mutating ctime_nsec must invalidate hash"
        );

        // 6. Mutate content_guard
        let mut p = plan.clone();
        p.items[0].content_guard = Some(ContentGuard {
            algorithm: "BLAKE3".into(),
            full_digest: "abcd1234".into(),
        });
        assert!(
            p.verify_integrity().is_err(),
            "mutating content_guard must invalidate hash"
        );

        // 7. Mutate preservation guard
        let mut p = plan.clone();
        p.preservation_guards.push(PreservationGuard {
            path: PathBuf::from("/tmp/keep"),
            expected_device_id: 1,
            expected_inode: 2,
            expected_size: 100,
            expected_mtime_sec: 1,
            expected_mtime_nsec: 2,
            expected_ctime_sec: 3,
            expected_ctime_nsec: 4,
            content_algorithm: "BLAKE3".into(),
            full_digest: "keepdigest".into(),
        });
        assert!(
            p.verify_integrity().is_err(),
            "adding preservation guard must invalidate hash"
        );
    }

    #[test]
    fn test_legacy_v1_plan_refused_for_destructive_execution() {
        let f = NamedTempFile::new().unwrap();
        let cand = make_test_candidate(
            f.path().to_path_buf(),
            RiskLevel::Safe,
            CandidateCategory::Cache,
        );
        let mut plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "rules-v1").unwrap();

        // Convert to legacy v1 plan
        plan.plan_schema_version = 1;
        plan.plan_hash = compute_plan_hash_v1(&plan.items, &plan.created_at);

        // Inspection / integrity verification succeeds
        assert!(plan.verify_integrity().is_ok());

        // Destructive execution check MUST FAIL!
        let exec_res = plan.verify_for_destructive_execution();
        assert!(exec_res.is_err());
        match exec_res.err().unwrap() {
            ReclaimError::PlanExecutionRefused { reason } => {
                assert!(
                    reason.contains("Legacy plan schema is not eligible for destructive execution")
                );
            }
            other => panic!("Unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_duplicate_plan_without_preservation_guard_refused() {
        let f = NamedTempFile::new().unwrap();
        let cand = make_test_candidate(
            f.path().to_path_buf(),
            RiskLevel::Review,
            CandidateCategory::Duplicate,
        );
        let plan = CleanupPlan::build(&[cand], RiskLevel::Review, "rules-v1").unwrap();

        // Integrity ok, but duplicate plan lacking preservation guard MUST FAIL destructive check
        assert!(plan.verify_integrity().is_ok());
        let res = plan.verify_for_destructive_execution();
        assert!(res.is_err());
        match res.err().unwrap() {
            ReclaimError::PlanExecutionRefused { reason } => {
                assert!(reason.contains("missing required preservation guard"));
            }
            other => panic!("Unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_stale_plan_preflight_refusal() {
        use std::io::Write;
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "initial content").unwrap();
        f.flush().unwrap();

        let meta = std::fs::symlink_metadata(f.path()).unwrap();
        let cand = Candidate {
            id: "cand-real".to_string(),
            path: f.path().to_path_buf(),
            category: CandidateCategory::Cache,
            allocation: AllocationInfo::new(meta.len(), meta.blocks() * 512, false),
            risk: RiskLevel::Safe,
            value: RecommendationValue::High,
            confidence_score: 1.0,
            evidence: vec![],
            reconstructable: true,
            rebuild_consequence: None,
            inode: meta.ino(),
            device_id: meta.dev(),
            mtime_sec: meta.mtime(),
            mtime_nsec: meta.mtime_nsec() as i64,
            ctime_sec: meta.ctime(),
            ctime_nsec: meta.ctime_nsec() as i64,
        };

        let plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "rules-v1").unwrap();
        assert!(
            !plan.is_stale(),
            "Freshly compiled plan with matching metadata must not be stale"
        );
        assert!(
            plan.preflight_check().is_ok(),
            "Fresh plan passes preflight"
        );

        // Mutate the target file (change size and mtime)
        writeln!(
            f,
            "modified content changed significantly to trigger toctou mismatch"
        )
        .unwrap();
        f.flush().unwrap();

        // Invariant: Plan is now stale!
        assert!(
            plan.is_stale(),
            "Plan must be identified as stale after file mutation"
        );
        let preflight = plan.preflight_check();
        assert!(
            preflight.is_err(),
            "Preflight check must refuse mutated stale target"
        );
    }
}
