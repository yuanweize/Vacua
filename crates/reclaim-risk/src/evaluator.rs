use sha2::{Digest, Sha256};
use std::path::Path;

use reclaim_core::allocation::AllocationInfo;
use reclaim_core::candidate::{Candidate, CandidateCategory};
use reclaim_core::evidence::{Evidence, EvidenceSource};
use reclaim_core::invariants::{enforce_safety_invariants, is_protected_path};
use reclaim_core::risk::{RecommendationValue, RiskLevel};
use reclaim_rules::engine::RulesEngine;

pub struct CandidateEvaluator<'a> {
    rules_engine: &'a mut RulesEngine,
}

impl<'a> CandidateEvaluator<'a> {
    pub fn new(rules_engine: &'a mut RulesEngine) -> Self {
        Self { rules_engine }
    }

    /// Evaluate an item into a fully scored, invariant-checked Candidate.
    pub fn evaluate(
        &mut self,
        path: &Path,
        allocation: AllocationInfo,
        inode: u64,
        device_id: u64,
        mtime_sec: i64,
        is_dir: bool,
    ) -> Candidate {
        let mut evidence = Vec::new();
        let canonical_str = path.to_string_lossy().to_string();

        // 1. Check Hard Protected Boundary Invariant
        if is_protected_path(path) {
            evidence.push(Evidence::new(
                EvidenceSource::PathSemantic,
                "matches hard-coded protected system or credential path",
                1.0,
                "Critical system location or sensitive user data; cannot be deleted.",
            ));

            let id = compute_candidate_id(&canonical_str);
            return Candidate {
                id,
                path: path.to_path_buf(),
                category: CandidateCategory::UserDocument,
                allocation,
                risk: RiskLevel::Protected,
                value: compute_value(allocation.allocated_bytes),
                confidence_score: 1.0,
                evidence,
                reconstructable: false,
                rebuild_consequence: None,
                inode,
                device_id,
                mtime_sec,
            };
        }

        // 2. Rule Engine Matching
        let matched_rule = self
            .rules_engine
            .match_path(path, mtime_sec, is_dir)
            .cloned();

        let (category, mut risk, reconstructable, rebuild_consequence, confidence_score) =
            if let Some(rule) = matched_rule {
                evidence.push(Evidence::new(
                    EvidenceSource::RuleEngine,
                    format!("matched rule: {}", rule.id),
                    0.95,
                    format!("{}: {}", rule.name, rule.description),
                ));

                if rule.reconstructable {
                    evidence.push(Evidence::new(
                        EvidenceSource::Reconstructability,
                        "reproducible build or cache artifact",
                        0.9,
                        rule.rebuild_consequence
                            .clone()
                            .unwrap_or_else(|| "Can be recreated by application.".into()),
                    ));
                }

                (
                    rule.category,
                    rule.risk,
                    rule.reconstructable,
                    rule.rebuild_consequence,
                    0.95,
                )
            } else {
                // Unrecognized file
                evidence.push(Evidence::new(
                    EvidenceSource::PathSemantic,
                    "unrecognized path pattern",
                    0.5,
                    "No rule or known application pattern matches this item.",
                ));

                (
                    CandidateCategory::Unknown,
                    RiskLevel::Unknown,
                    false,
                    None,
                    0.3,
                )
            };

        // 3. Double-check Invariants: if Category is Unknown, risk MUST NOT be Safe
        if category == CandidateCategory::Unknown && risk == RiskLevel::Safe {
            risk = RiskLevel::Unknown;
        }

        let value = compute_value(allocation.allocated_bytes);
        let id = compute_candidate_id(&canonical_str);

        let candidate = Candidate {
            id,
            path: path.to_path_buf(),
            category,
            allocation,
            risk,
            value,
            confidence_score,
            evidence,
            reconstructable,
            rebuild_consequence,
            inode,
            device_id,
            mtime_sec,
        };

        // Enforce invariants on the constructed candidate
        let _ = enforce_safety_invariants(&candidate);

        candidate
    }
}

fn compute_value(allocated_bytes: u64) -> RecommendationValue {
    const GIB: u64 = 1024 * 1024 * 1024;
    const MIB: u64 = 1024 * 1024;

    if allocated_bytes >= 5 * GIB {
        RecommendationValue::High
    } else if allocated_bytes >= GIB {
        RecommendationValue::Medium
    } else if allocated_bytes >= 50 * MIB {
        RecommendationValue::Low
    } else {
        RecommendationValue::Negligible
    }
}

fn compute_candidate_id(path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..6]) // 12-char hex ID
}

mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_evaluates_xcode_derived_data_as_safe() {
        let mut engine = RulesEngine::new();
        let mut evaluator = CandidateEvaluator::new(&mut engine);

        let path = dirs_home().join("Library/Developer/Xcode/DerivedData/MyApp-abcdef");
        let alloc = AllocationInfo::new(100_000_000, 100_000_000, false);
        let cand = evaluator.evaluate(&path, alloc, 123, 456, 0, true);

        assert_eq!(cand.category, CandidateCategory::BuildArtifact);
        assert_eq!(cand.risk, RiskLevel::Safe);
        assert!(cand.reconstructable);
        assert_eq!(cand.value, RecommendationValue::Low);
        assert!(cand.is_auto_cleanable());
    }

    #[test]
    fn test_evaluates_ssh_key_as_protected() {
        let mut engine = RulesEngine::new();
        let mut evaluator = CandidateEvaluator::new(&mut engine);

        let path = dirs_home().join(".ssh/id_ed25519");
        let alloc = AllocationInfo::new(4096, 4096, false);
        let cand = evaluator.evaluate(&path, alloc, 111, 222, 0, false);

        assert_eq!(cand.risk, RiskLevel::Protected);
        assert!(!cand.is_auto_cleanable());
    }

    #[test]
    fn test_evaluates_unknown_file_as_unknown_risk() {
        let mut engine = RulesEngine::new();
        let mut evaluator = CandidateEvaluator::new(&mut engine);

        let path = PathBuf::from("/tmp/random_unclassified_file.xyz");
        let alloc = AllocationInfo::new(1024, 1024, false);
        let cand = evaluator.evaluate(&path, alloc, 333, 444, 0, false);

        assert_eq!(cand.category, CandidateCategory::Unknown);
        assert_eq!(cand.risk, RiskLevel::Unknown);
        assert!(!cand.is_auto_cleanable());
    }

    fn dirs_home() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/Users/testuser"))
    }
}
