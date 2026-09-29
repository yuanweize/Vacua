use crate::cache::FingerprintCache;
use crate::cloud::is_cloud_placeholder;
use crate::group::{DuplicateGroup, DuplicateMember};
use crate::identity::{
    compute_full_fingerprint_toctou, compute_sample_fingerprint, ContentError, FingerprintState,
    PhysicalRelation,
};
use crate::stats::DedupStats;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use vacua_core::RiskLevel;
use vacua_scan::ScannedEntry;

#[derive(Debug, Clone)]
pub struct DuplicateScanOptions {
    pub min_size: u64,
    pub include_empty: bool,
    pub hash_jobs: usize,
    pub skip_cloud: bool,
    pub use_cache: bool,
}

impl Default for DuplicateScanOptions {
    fn default() -> Self {
        Self {
            min_size: 1024 * 1024, // 1 MiB default
            include_empty: false,
            hash_jobs: 4,
            skip_cloud: true,
            use_cache: true,
        }
    }
}

/// Runs the 6-stage duplicate detection pipeline over scanned filesystem entries.
pub fn run_staged_duplicate_pipeline<F>(
    entries: &[ScannedEntry],
    options: &DuplicateScanOptions,
    cache: Option<&FingerprintCache>,
    mut risk_classifier: F,
) -> Result<(Vec<DuplicateGroup>, DedupStats), ContentError>
where
    F: FnMut(&ScannedEntry) -> (RiskLevel, String),
{
    let mut stats = DedupStats {
        files_seen: entries.len() as u64,
        ..Default::default()
    };

    // Stage 0: Eligibility filtering
    let mut eligible: Vec<&ScannedEntry> = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.is_dir || entry.is_symlink {
            continue;
        }

        if entry.logical_bytes == 0 && !options.include_empty {
            continue;
        }

        if entry.logical_bytes < options.min_size {
            continue;
        }

        if options.skip_cloud && is_cloud_placeholder(&entry.path) {
            stats.cloud_placeholders_skipped += 1;
            continue;
        }

        eligible.push(entry);
    }
    stats.eligible_files = eligible.len() as u64;

    // Stage 1: Size Buckets
    let mut size_buckets: HashMap<u64, Vec<&ScannedEntry>> = HashMap::new();
    for entry in eligible {
        size_buckets
            .entry(entry.logical_bytes)
            .or_default()
            .push(entry);
    }

    let mut collision_entries: Vec<&ScannedEntry> = Vec::new();
    for (_size, group) in size_buckets {
        if group.len() < 2 {
            stats.size_unique_files += 1;
        } else {
            stats.size_collision_files += group.len() as u64;
            collision_entries.extend(group);
        }
    }

    if collision_entries.is_empty() {
        return Ok((Vec::new(), stats));
    }

    // Stage 2: Filesystem Identity Collapse & Hardlink tracking
    let mut hardlink_tracker: HashMap<(u64, u64), u64> = HashMap::new();
    for entry in &collision_entries {
        *hardlink_tracker
            .entry((entry.device_id, entry.inode))
            .or_default() += 1;
    }
    for count in hardlink_tracker.values() {
        if *count > 1 {
            stats.hardlinks_collapsed += count - 1;
        }
    }

    // Stage 3: APFS Clone tracking
    for entry in &collision_entries {
        if entry.is_clone || entry.clone_id.is_some() {
            stats.clone_family_members += 1;
        }
    }

    // Stage 4: Sample Fingerprint calculation / cache lookup
    let mut sample_groups: HashMap<(u64, String), Vec<&ScannedEntry>> = HashMap::new();
    for entry in collision_entries {
        let mut sample_hash = None;

        if options.use_cache {
            if let Some(c) = cache {
                if let Ok(Some(cached)) = c.get(
                    entry.device_id,
                    entry.inode,
                    &entry.path,
                    entry.logical_bytes,
                    entry.mtime_sec,
                    entry.mtime_nsec,
                    entry.ctime_sec,
                    entry.ctime_nsec,
                ) {
                    if let Some(s) = cached.sample_hash {
                        sample_hash = Some(s);
                        stats.full_hash_cache_hits += 1;
                    }
                }
            }
        }

        let digest = match sample_hash {
            Some(h) => h,
            None => {
                let (computed, state, read) =
                    compute_sample_fingerprint(&entry.path, entry.logical_bytes)?;
                if state == FingerprintState::SkippedCloudPlaceholder {
                    stats.cloud_placeholders_skipped += 1;
                    continue;
                }
                if state == FingerprintState::ChangedDuringRead {
                    continue;
                }
                stats.sampled_files += 1;
                stats.sample_bytes_read += read;

                if options.use_cache {
                    if let Some(c) = cache {
                        let _ = c.put_sample(
                            entry.device_id,
                            entry.inode,
                            &entry.path,
                            entry.logical_bytes,
                            entry.mtime_sec,
                            entry.mtime_nsec,
                            entry.ctime_sec,
                            entry.ctime_nsec,
                            &computed,
                        );
                    }
                }
                computed
            }
        };

        sample_groups
            .entry((entry.logical_bytes, digest))
            .or_default()
            .push(entry);
    }

    // Retain sample groups with >= 2 candidates
    let mut candidate_full_entries: Vec<&ScannedEntry> = Vec::new();
    for (_key, group) in sample_groups {
        if group.len() >= 2 {
            candidate_full_entries.extend(group);
        }
    }

    if candidate_full_entries.is_empty() {
        return Ok((Vec::new(), stats));
    }

    // Stage 5: Full BLAKE3 digest calculation / cache lookup
    let mut full_groups: HashMap<(u64, String), Vec<&ScannedEntry>> = HashMap::new();
    for entry in candidate_full_entries {
        let mut full_hash = None;

        if options.use_cache {
            if let Some(c) = cache {
                if let Ok(Some(cached)) = c.get(
                    entry.device_id,
                    entry.inode,
                    &entry.path,
                    entry.logical_bytes,
                    entry.mtime_sec,
                    entry.mtime_nsec,
                    entry.ctime_sec,
                    entry.ctime_nsec,
                ) {
                    if let Some(f) = cached.full_hash {
                        full_hash = Some(f);
                        stats.full_hash_cache_hits += 1;
                    }
                }
            }
        }

        let digest = match full_hash {
            Some(h) => h,
            None => {
                let (computed, state, read) = compute_full_fingerprint_toctou(
                    &entry.path,
                    entry.device_id,
                    entry.inode,
                    entry.logical_bytes,
                    entry.mtime_sec,
                    entry.mtime_nsec,
                    entry.ctime_sec,
                    entry.ctime_nsec,
                )?;

                if state != FingerprintState::FullHash {
                    continue;
                }
                stats.full_hashed_files += 1;
                stats.full_hash_bytes_read += read;
                stats.full_hash_cache_misses += 1;

                if options.use_cache {
                    if let Some(c) = cache {
                        let _ = c.put_full(
                            entry.device_id,
                            entry.inode,
                            &entry.path,
                            entry.logical_bytes,
                            entry.mtime_sec,
                            entry.mtime_nsec,
                            entry.ctime_sec,
                            entry.ctime_nsec,
                            None,
                            &computed,
                        );
                    }
                }
                computed
            }
        };

        full_groups
            .entry((entry.logical_bytes, digest))
            .or_default()
            .push(entry);
    }

    // Construct final DuplicateGroup objects
    let mut duplicate_groups = Vec::new();
    for ((logical_size, digest), members_entries) in full_groups {
        if members_entries.len() < 2 {
            continue;
        }

        let members: Vec<DuplicateMember> = members_entries
            .into_iter()
            .map(|e| {
                let (risk, category) = risk_classifier(e);
                DuplicateMember {
                    path: e.path.clone(),
                    device_id: e.device_id,
                    inode: e.inode,
                    allocation: e.to_allocation(),
                    clone_id: e.clone_id,
                    clone_refcnt: e.clone_refcnt,
                    nlink: e.nlink,
                    risk,
                    category,
                    mtime_sec: e.mtime_sec,
                    physical_relation: PhysicalRelation::Independent,
                    is_suggested_keep: false,
                    suggest_keep_reason: None,
                }
            })
            .collect();

        let group = DuplicateGroup::new(logical_size, digest, members);
        stats.logical_duplicate_bytes += group.logical_duplicate_bytes;
        stats.confirmed_reclaimable_bytes += group.confirmed_reclaimable_bytes;
        stats.estimated_reclaimable_bytes += group.estimated_reclaimable_bytes;
        stats.upper_bound_reclaimable_bytes += group.upper_bound_reclaimable_bytes;
        stats.duplicate_members += group.members.len();

        duplicate_groups.push(group);
    }

    // Deterministic sort: largest logical duplicate size first, then group_id
    duplicate_groups.sort_by(|a, b| {
        b.logical_duplicate_bytes
            .cmp(&a.logical_duplicate_bytes)
            .then_with(|| a.group_id.cmp(&b.group_id))
    });

    stats.duplicate_groups = duplicate_groups.len();
    Ok((duplicate_groups, stats))
}

/// Stage 6: Destructive Confirmation
/// Revalidates that both the member to keep and the member to delete still exist,
/// have identical sizes, and have matching cryptographic content hashes before
/// any cleanup plan is finalized.
pub fn verify_duplicate_pair_before_deletion(
    keep_path: &Path,
    delete_path: &Path,
) -> Result<bool, ContentError> {
    let mut keep_file = File::open(keep_path)?;
    let mut delete_file = File::open(delete_path)?;

    let meta_keep = keep_file.metadata()?;
    let meta_delete = delete_file.metadata()?;

    if !meta_keep.is_file() || !meta_delete.is_file() {
        return Ok(false);
    }

    if meta_keep.len() != meta_delete.len() {
        return Ok(false);
    }

    // Stream and compare byte-for-byte in chunks
    let mut buf_keep = [0u8; 64 * 1024];
    let mut buf_del = [0u8; 64 * 1024];

    loop {
        let n_keep = keep_file.read(&mut buf_keep)?;
        let n_del = delete_file.read(&mut buf_del)?;

        if n_keep != n_del {
            return Ok(false);
        }
        if n_keep == 0 {
            break;
        }
        if buf_keep[..n_keep] != buf_del[..n_del] {
            return Ok(false);
        }
    }

    // Post-read fstat verification to prevent TOCTOU substitution
    let post_keep = keep_file.metadata()?;
    let post_del = delete_file.metadata()?;

    if post_keep.mtime() != meta_keep.mtime()
        || post_keep.mtime_nsec() != meta_keep.mtime_nsec()
        || post_del.mtime() != meta_delete.mtime()
        || post_del.mtime_nsec() != meta_delete.mtime_nsec()
    {
        return Err(ContentError::ChangedDuringRead);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_staged_pipeline_discovers_identical_files() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("file1.bin");
        let file2 = dir.path().join("file2.bin");
        let file3 = dir.path().join("different.bin");

        let content = vec![0xAB; 200 * 1024]; // 200 KiB
        std::fs::write(&file1, &content).unwrap();
        std::fs::write(&file2, &content).unwrap();
        std::fs::write(&file3, vec![0xCD; 200 * 1024]).unwrap();

        let entries = vec![
            ScannedEntry::from_path(file1).unwrap(),
            ScannedEntry::from_path(file2).unwrap(),
            ScannedEntry::from_path(file3).unwrap(),
        ];

        let options = DuplicateScanOptions {
            min_size: 100 * 1024,
            include_empty: false,
            hash_jobs: 2,
            skip_cloud: true,
            use_cache: false,
        };

        let (groups, stats) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
            (RiskLevel::Review, "test".to_string())
        })
        .unwrap();

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members.len(), 2);
        assert_eq!(stats.duplicate_groups, 1);
        assert_eq!(stats.size_collision_files, 3);
    }

    #[test]
    fn test_verify_duplicate_pair_byte_for_byte() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("a.bin");
        let f2 = dir.path().join("b.bin");
        let f3 = dir.path().join("c.bin");

        std::fs::write(&f1, b"exact byte match").unwrap();
        std::fs::write(&f2, b"exact byte match").unwrap();
        std::fs::write(&f3, b"different byte match").unwrap();

        assert!(verify_duplicate_pair_before_deletion(&f1, &f2).unwrap());
        assert!(!verify_duplicate_pair_before_deletion(&f1, &f3).unwrap());
    }
}
