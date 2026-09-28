use crate::candidate::{Candidate, CandidateCategory};
use crate::error::{ReclaimError, Result};
use crate::risk::RiskLevel;
use std::path::Path;

/// Check if a given canonical path resides within an inviolable protected system or user location.
pub fn is_protected_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // 1. Root & System OS directories
    if path_str == "/"
        || path_str == "/System"
        || path_str.starts_with("/System/")
        || path_str == "/usr"
        || path_str.starts_with("/usr/bin")
        || path_str.starts_with("/usr/sbin")
        || path_str.starts_with("/bin")
        || path_str.starts_with("/sbin")
        || path_str.starts_with("/private/var/db")
    {
        return true;
    }

    // 2. Time Machine mounts & local snapshots
    if path_str.contains("/Backups.backupdb/")
        || path_str.starts_with("/Volumes/Time Machine")
        || path_str.starts_with("/Volumes/.timemachine")
    {
        return true;
    }

    // 3. User security keys, credentials, and identity
    if path_str.contains("/.ssh")
        || path_str.contains("/.gnupg")
        || path_str.contains("/Library/Keychains")
        || path_str.ends_with("/.aws/credentials")
        || path_str.ends_with("/.kube/config")
    {
        return true;
    }

    // 4. Primary user document, photos, and mail databases
    if path_str.contains("/Library/Mail")
        || path_str.contains("/Library/Messages")
        || path_str.contains("/Library/Safari")
        || path_str.ends_with(".photoslibrary")
        || path_str.contains(".photoslibrary/")
    {
        return true;
    }

    // 5. Active git repositories (.git directory itself)
    if path_str.ends_with("/.git") || path_str.contains("/.git/") {
        return true;
    }

    false
}

/// Enforce inviolable compile-time and run-time safety invariants on any candidate.
/// Returns an error if any invariant is violated.
pub fn enforce_safety_invariants(candidate: &Candidate) -> Result<()> {
    // Invariant 1: Protected paths must never be marked Safe or auto-cleaned
    if is_protected_path(&candidate.path)
        && (candidate.risk == RiskLevel::Safe || candidate.is_auto_cleanable())
    {
        return Err(ReclaimError::SafetyViolation(format!(
            "PROTECTED invariant breached: path {:?} cannot be SAFE or auto-cleaned",
            candidate.path
        )));
    }

    // Invariant 2: UNKNOWN category must NEVER be Safe or auto-cleaned
    if candidate.category == CandidateCategory::Unknown
        && (candidate.risk == RiskLevel::Safe || candidate.is_auto_cleanable())
    {
        return Err(ReclaimError::SafetyViolation(format!(
            "UNKNOWN invariant breached: candidate {:?} has UNKNOWN category and cannot be auto-cleaned",
            candidate.path
        )));
    }

    // Invariant 3: Reclaimable bytes cannot physically exceed allocated bytes
    if candidate.allocation.potentially_reclaimable_bytes > candidate.allocation.allocated_bytes {
        return Err(ReclaimError::SafetyViolation(format!(
            "Accounting invariant breached: reclaimable bytes ({}) cannot exceed allocated bytes ({})",
            candidate.allocation.potentially_reclaimable_bytes,
            candidate.allocation.allocated_bytes
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocation::AllocationInfo;
    use crate::risk::RecommendationValue;
    use std::path::PathBuf;

    #[test]
    fn test_protected_paths() {
        assert!(is_protected_path(Path::new("/System/Library")));
        assert!(is_protected_path(Path::new("/usr/bin/login")));
        assert!(is_protected_path(Path::new("/Users/foo/.ssh/id_rsa")));
        assert!(is_protected_path(Path::new(
            "/Users/foo/Library/Keychains/login.keychain-db"
        )));
        assert!(is_protected_path(Path::new(
            "/Users/foo/Pictures/Photos Library.photoslibrary/database"
        )));
        assert!(is_protected_path(Path::new(
            "/Users/foo/code/repo/.git/HEAD"
        )));
        assert!(!is_protected_path(Path::new(
            "/Users/foo/Library/Caches/com.apple.dt.Xcode"
        )));
    }

    #[test]
    fn test_invariant_enforcement_blocks_protected_safe() {
        let candidate = Candidate {
            id: "test1".into(),
            path: PathBuf::from("/Users/foo/.ssh/id_rsa"),
            category: CandidateCategory::Cache,
            allocation: AllocationInfo::new(100, 100, false),
            risk: RiskLevel::Safe, // VIOLATION!
            value: RecommendationValue::High,
            confidence_score: 0.9,
            evidence: vec![],
            reconstructable: false,
            rebuild_consequence: None,
            inode: 1,
            device_id: 1,
            mtime_sec: 0,
        };

        assert!(enforce_safety_invariants(&candidate).is_err());
    }

    #[test]
    fn test_invariant_enforcement_blocks_unknown_safe() {
        let candidate = Candidate {
            id: "test2".into(),
            path: PathBuf::from("/Users/foo/Library/Caches/RandomUnknownThing"),
            category: CandidateCategory::Unknown, // UNKNOWN
            allocation: AllocationInfo::new(100, 100, false),
            risk: RiskLevel::Safe, // VIOLATION!
            value: RecommendationValue::Medium,
            confidence_score: 0.5,
            evidence: vec![],
            reconstructable: false,
            rebuild_consequence: None,
            inode: 2,
            device_id: 1,
            mtime_sec: 0,
        };

        assert!(enforce_safety_invariants(&candidate).is_err());
    }
}
