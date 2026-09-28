use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;

use vacua_core::candidate::{Candidate, CandidateCategory};
use vacua_core::error::{ReclaimError, Result};
use vacua_core::risk::RiskLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub candidate_id: String,
    pub path: PathBuf,
    pub category: CandidateCategory,
    pub risk: RiskLevel,
    pub allocated_bytes: u64,
    pub expected_inode: u64,
    pub expected_device_id: u64,
    pub expected_mtime_sec: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub plan_id: String,
    pub created_at: DateTime<Utc>,
    pub ruleset_version: String,
    pub items: Vec<PlanItem>,
    pub estimated_physical_reclaim: u64,
    pub risk_summary: PlanRiskSummary,
    pub plan_hash: String,
}

impl CleanupPlan {
    pub fn build(
        candidates: &[Candidate],
        max_allowed_risk: RiskLevel,
        ruleset_version: &str,
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
                expected_mtime_sec: cand.mtime_sec,
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
        let plan_hash = compute_plan_hash(&items, &now);
        let plan_id = format!("plan-{}", &plan_hash[..12]);

        Ok(Self {
            plan_id,
            created_at: now,
            ruleset_version: ruleset_version.to_string(),
            items,
            estimated_physical_reclaim: total_reclaim,
            risk_summary,
            plan_hash,
        })
    }

    /// Re-verify plan integrity against its embedded SHA-256 hash.
    pub fn verify_integrity(&self) -> Result<()> {
        let computed = compute_plan_hash(&self.items, &self.created_at);
        if computed != self.plan_hash {
            return Err(ReclaimError::PlanIntegrityFailure {
                expected: self.plan_hash.clone(),
                computed,
            });
        }
        Ok(())
    }

    /// Verify an individual item against filesystem state immediately before execution (TOCTOU defense).
    pub fn verify_toctou(item: &PlanItem) -> Result<()> {
        let path = &item.path;
        if !path.exists() {
            return Err(ReclaimError::ToctouMismatch {
                path: path.display().to_string(),
                reason: "path no longer exists".into(),
            });
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta =
                std::fs::symlink_metadata(path).map_err(|e| ReclaimError::ToctouMismatch {
                    path: path.display().to_string(),
                    reason: format!("failed to read metadata: {}", e),
                })?;

            // Defense: Target was swapped with a symlink!
            if meta.file_type().is_symlink() {
                return Err(ReclaimError::ToctouMismatch {
                    path: path.display().to_string(),
                    reason: "target was substituted with a symlink".into(),
                });
            }

            // Inode match check
            if meta.ino() != item.expected_inode {
                return Err(ReclaimError::ToctouMismatch {
                    path: path.display().to_string(),
                    reason: format!(
                        "inode mismatch (expected {}, found {})",
                        item.expected_inode,
                        meta.ino()
                    ),
                });
            }

            // Device match check
            if meta.dev() != item.expected_device_id {
                return Err(ReclaimError::ToctouMismatch {
                    path: path.display().to_string(),
                    reason: format!(
                        "device mismatch (expected {}, found {})",
                        item.expected_device_id,
                        meta.dev()
                    ),
                });
            }
        }

        Ok(())
    }
}

fn compute_plan_hash(items: &[PlanItem], created_at: &DateTime<Utc>) -> String {
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
    use std::fs::File;
    use tempfile::tempdir;
    use vacua_core::allocation::AllocationInfo;
    use vacua_core::risk::RecommendationValue;

    #[test]
    fn test_plan_building_and_integrity_check() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("dummy.cache");
        File::create(&file_path).unwrap();

        let meta = std::fs::symlink_metadata(&file_path).unwrap();
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let cand = Candidate {
            id: "cand1".into(),
            path: file_path.clone(),
            category: CandidateCategory::Cache,
            allocation: AllocationInfo::new(1024, 1024, false),
            risk: RiskLevel::Safe,
            value: RecommendationValue::Low,
            confidence_score: 0.95,
            evidence: vec![],
            reconstructable: true,
            rebuild_consequence: None,
            inode: meta.ino(),
            device_id: meta.dev(),
            mtime_sec: meta.mtime(),
        };

        let plan = CleanupPlan::build(&[cand], RiskLevel::Safe, "1.0").unwrap();
        assert_eq!(plan.items.len(), 1);
        assert!(plan.verify_integrity().is_ok());

        // Test TOCTOU succeeds when file is unchanged
        assert!(CleanupPlan::verify_toctou(&plan.items[0]).is_ok());

        // Test TOCTOU detects removal
        std::fs::remove_file(&file_path).unwrap();
        assert!(CleanupPlan::verify_toctou(&plan.items[0]).is_err());
    }
}
