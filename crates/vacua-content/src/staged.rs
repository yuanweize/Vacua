use crate::cache::FingerprintCache;
use crate::cloud::is_cloud_placeholder;
use crate::group::{DuplicateGroup, DuplicateMember};
use crate::identity::{
    compute_full_fingerprint_toctou, compute_sample_fingerprint, ContentError, FingerprintState,
    PhysicalRelation,
};
use crate::stats::DedupStats;
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use vacua_core::fs::{open_regular_file_safely, FileKind};
use vacua_core::RiskLevel;
use vacua_scan::ScannedEntry;

// Atomic probe for testing worker concurrency
pub static PEAK_HASH_WORKERS: AtomicUsize = AtomicUsize::new(0);

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

/// Helper struct for representative ContentObject (hardlink collapse)
struct ContentRepresentative<'a> {
    entry: &'a ScannedEntry,
    aliases: Vec<&'a ScannedEntry>,
}

fn run_parallel_map<T: Send + 'static, R: Send + 'static, F>(
    items: Vec<T>,
    num_workers: usize,
    f: F,
) -> Vec<R>
where
    F: Fn(T) -> R + Send + Sync + 'static,
{
    if items.is_empty() {
        return Vec::new();
    }
    if num_workers <= 1 || items.len() <= 1 {
        return items.into_iter().map(f).collect();
    }

    use std::sync::mpsc;
    use std::thread;

    let (item_tx, item_rx) = mpsc::sync_channel::<T>(num_workers * 2);
    let (res_tx, res_rx) = mpsc::channel();
    let f = Arc::new(f);

    let active_counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(num_workers);
    let item_rx = Arc::new(std::sync::Mutex::new(item_rx));

    for _ in 0..num_workers {
        let rx = Arc::clone(&item_rx);
        let tx = res_tx.clone();
        let f_clone = Arc::clone(&f);
        let counter = Arc::clone(&active_counter);

        let handle = thread::spawn(move || {
            loop {
                let item = {
                    let lock = rx.lock().unwrap();
                    match lock.recv() {
                        Ok(item) => item,
                        Err(_) => break, // Channel closed
                    }
                };
                let prev = counter.fetch_add(1, Ordering::SeqCst);
                let current_active = prev + 1;
                // Update global peak active workers probe
                let mut peak = PEAK_HASH_WORKERS.load(Ordering::Relaxed);
                while current_active > peak {
                    match PEAK_HASH_WORKERS.compare_exchange_weak(
                        peak,
                        current_active,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(actual) => peak = actual,
                    }
                }

                let res = f_clone(item);
                counter.fetch_sub(1, Ordering::SeqCst);

                if tx.send(res).is_err() {
                    break;
                }
            }
        });
        handles.push(handle);
    }
    drop(res_tx); // Drop extra sender

    let total_items = items.len();
    thread::spawn(move || {
        for item in items {
            if item_tx.send(item).is_err() {
                break;
            }
        }
    });

    let mut results = Vec::with_capacity(total_items);
    for res in res_rx {
        results.push(res);
    }

    for h in handles {
        let _ = h.join();
    }

    results
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

    // Stage 0: Eligibility filtering (strictly regular files only)
    let mut eligible: Vec<&ScannedEntry> = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.file_kind != FileKind::Regular || entry.is_dir || entry.is_symlink {
            stats.special_files_skipped += 1;
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

    // Stage 2: Filesystem Identity Collapse & Hardlink Tracking
    // Real hardlink collapsing: Group identical (dev, inode) entries together.
    // Hash only the representative entry once, avoiding redundant reads for hardlinks.
    let mut hardlink_groups: HashMap<(u64, u64), Vec<&ScannedEntry>> = HashMap::new();
    for entry in collision_entries {
        hardlink_groups
            .entry((entry.device_id, entry.inode))
            .or_default()
            .push(entry);
    }

    let mut representatives: Vec<ContentRepresentative> = Vec::new();
    for (_dev_ino, group) in hardlink_groups {
        let count = group.len();
        if count > 1 {
            stats.hardlinks_collapsed += (count - 1) as u64;
            stats.hardlink_aliases_collapsed += (count - 1) as u64;
        }
        representatives.push(ContentRepresentative {
            entry: group[0],
            aliases: group[1..].to_vec(),
        });
    }

    // Stage 3: APFS Clone tracking
    for rep in &representatives {
        if rep.entry.is_clone || rep.entry.clone_id.is_some() {
            stats.clone_family_members += 1 + rep.aliases.len() as u64;
        }
    }

    // Stage 4: Sample Fingerprint calculation with bounded worker pool
    let mut sample_hashes: HashMap<usize, String> = HashMap::new();
    let mut direct_full_hashes: HashMap<usize, String> = HashMap::new();

    // 4.1 Check cache first on main thread
    let mut reps_to_hash = Vec::new();
    for (idx, rep) in representatives.iter().enumerate() {
        let entry = rep.entry;
        let mut hit = false;
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
                        sample_hashes.insert(idx, s);
                        stats.sample_cache_hits += 1;
                        hit = true;
                    }
                    if let Some(f) = cached.full_hash {
                        direct_full_hashes.insert(idx, f);
                        stats.full_cache_hits += 1;
                        stats.full_hash_cache_hits += 1; // backward-compat stat
                    }
                }
            }
        }
        if !hit {
            stats.sample_cache_misses += 1;
            reps_to_hash.push((idx, entry.path.clone(), entry.logical_bytes));
        }
    }

    // 4.2 Hash uncached entries using bounded worker pool
    let num_workers = options.hash_jobs.max(1);
    let sample_results = run_parallel_map(
        reps_to_hash,
        num_workers,
        move |(idx, path, size)| -> (usize, Result<(String, FingerprintState, u64), ContentError>) {
            let res = compute_sample_fingerprint(&path, size);
            (idx, res)
        },
    );

    // 4.3 Collector: record results and write to cache serially
    for (idx, res) in sample_results {
        match res {
            Ok((computed, state, read)) => {
                if state == FingerprintState::SkippedCloudPlaceholder {
                    stats.cloud_placeholders_skipped += 1;
                    continue;
                }
                if state == FingerprintState::ChangedDuringRead {
                    stats.io_errors_skipped += 1;
                    continue;
                }

                stats.sampled_files += 1;
                stats.sample_bytes_read += read;

                // Account for hardlink aliases that avoided reading disk
                let alias_count = representatives[idx].aliases.len() as u64;
                if alias_count > 0 {
                    stats.hash_reads_avoided_by_hardlinks += alias_count;
                }

                let entry = representatives[idx].entry;
                sample_hashes.insert(idx, computed.clone());

                if state == FingerprintState::FullHash {
                    // Small file direct full hash! Record so Stage 5 skips reread
                    direct_full_hashes.insert(idx, computed.clone());

                    if options.use_cache {
                        if let Some(c) = cache {
                            if c.put_full(
                                entry.device_id,
                                entry.inode,
                                &entry.path,
                                entry.logical_bytes,
                                entry.mtime_sec,
                                entry.mtime_nsec,
                                entry.ctime_sec,
                                entry.ctime_nsec,
                                Some(&computed),
                                &computed,
                            )
                            .is_err()
                            {
                                stats.cache_write_failures += 1;
                            }
                        }
                    }
                } else if options.use_cache {
                    if let Some(c) = cache {
                        if c.put_sample(
                            entry.device_id,
                            entry.inode,
                            &entry.path,
                            entry.logical_bytes,
                            entry.mtime_sec,
                            entry.mtime_nsec,
                            entry.ctime_sec,
                            entry.ctime_nsec,
                            &computed,
                        )
                        .is_err()
                        {
                            stats.cache_write_failures += 1;
                        }
                    }
                }
            }
            Err(e) => {
                stats.io_errors_skipped += 1;
                // Resilient error handling: skip single broken file and continue
                let _ = e;
            }
        }
    }

    // Group representatives by (size, sample_hash)
    let mut sample_groups: HashMap<(u64, String), Vec<usize>> = HashMap::new();
    for (idx, hash) in sample_hashes {
        let size = representatives[idx].entry.logical_bytes;
        sample_groups.entry((size, hash)).or_default().push(idx);
    }

    // Filter for groups with >= 2 total files (considering representatives + aliases)
    let mut reps_needing_full = Vec::new();
    for (_key, group_rep_indices) in sample_groups {
        let total_files: usize = group_rep_indices
            .iter()
            .map(|&idx| 1 + representatives[idx].aliases.len())
            .sum();

        if total_files >= 2 {
            reps_needing_full.extend(group_rep_indices);
        }
    }

    if reps_needing_full.is_empty() {
        return Ok((Vec::new(), stats));
    }

    // Stage 5: Full BLAKE3 digest calculation with bounded worker pool
    let mut full_hashes: HashMap<usize, String> = HashMap::new();
    let mut reps_to_full_hash = Vec::new();

    for &idx in &reps_needing_full {
        let entry = representatives[idx].entry;

        // 5.1 Check if already direct-hashed in Stage 4 or in cache
        if let Some(h) = direct_full_hashes.get(&idx) {
            full_hashes.insert(idx, h.clone());
            continue;
        }

        let mut hit = false;
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
                        full_hashes.insert(idx, f);
                        stats.full_cache_hits += 1;
                        stats.full_hash_cache_hits += 1; // backward-compat stat
                        hit = true;
                    }
                }
            }
        }

        if !hit {
            stats.full_cache_misses += 1;
            stats.full_hash_cache_misses += 1; // backward-compat stat
            reps_to_full_hash.push((
                idx,
                entry.path.clone(),
                entry.device_id,
                entry.inode,
                entry.logical_bytes,
                entry.mtime_sec,
                entry.mtime_nsec,
                entry.ctime_sec,
                entry.ctime_nsec,
            ));
        }
    }

    // 5.2 Compute full hashes in parallel
    let full_results = run_parallel_map(
        reps_to_full_hash,
        num_workers,
        move |(idx, path, dev, ino, size, mtime_sec, mtime_nsec, ctime_sec, ctime_nsec)| -> (
            usize,
            Result<(String, FingerprintState, u64), ContentError>,
        ) {
            let res = compute_full_fingerprint_toctou(
                &path, dev, ino, size, mtime_sec, mtime_nsec, ctime_sec, ctime_nsec,
            );
            (idx, res)
        },
    );

    // 5.3 Collector for full hash results
    for (idx, res) in full_results {
        match res {
            Ok((computed, state, read)) => {
                if state != FingerprintState::FullHash {
                    stats.io_errors_skipped += 1;
                    continue;
                }

                stats.full_hashed_files += 1;
                stats.full_hash_bytes_read += read;

                let alias_count = representatives[idx].aliases.len() as u64;
                if alias_count > 0 {
                    stats.hash_reads_avoided_by_hardlinks += alias_count;
                }

                let entry = representatives[idx].entry;
                full_hashes.insert(idx, computed.clone());

                if options.use_cache {
                    if let Some(c) = cache {
                        if c.put_full(
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
                        )
                        .is_err()
                        {
                            stats.cache_write_failures += 1;
                        }
                    }
                }
            }
            Err(_) => {
                stats.io_errors_skipped += 1;
            }
        }
    }

    // Group by (logical_size, full_hash) including aliases
    let mut final_groups: HashMap<(u64, String), Vec<&ScannedEntry>> = HashMap::new();
    for (idx, digest) in full_hashes {
        let rep = &representatives[idx];
        let size = rep.entry.logical_bytes;
        let group_members = final_groups.entry((size, digest)).or_default();
        group_members.push(rep.entry);
        for alias in &rep.aliases {
            group_members.push(alias);
        }
    }

    // Construct final DuplicateGroup objects
    let mut duplicate_groups = Vec::new();
    for ((logical_size, digest), members_entries) in final_groups {
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
                    mtime_nsec: e.mtime_nsec,
                    ctime_sec: e.ctime_sec,
                    ctime_nsec: e.ctime_nsec,
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
/// are regular files, have identical sizes, and have matching cryptographic content hashes before
/// any cleanup plan is finalized.
pub fn verify_duplicate_pair_before_deletion(
    keep_path: &Path,
    delete_path: &Path,
) -> Result<bool, ContentError> {
    let mut keep_file = open_regular_file_safely(keep_path)?;
    let mut delete_file = open_regular_file_safely(delete_path)?;

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
    fn test_small_file_direct_full_hash_not_double_read() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("small1.bin");
        let file2 = dir.path().join("small2.bin");

        let content = vec![0x42; 100 * 1024]; // 100 KiB <= 192 KiB
        std::fs::write(&file1, &content).unwrap();
        std::fs::write(&file2, &content).unwrap();

        let entries = vec![
            ScannedEntry::from_path(file1).unwrap(),
            ScannedEntry::from_path(file2).unwrap(),
        ];

        let options = DuplicateScanOptions {
            min_size: 10 * 1024,
            include_empty: false,
            hash_jobs: 2,
            skip_cloud: true,
            use_cache: false,
        };

        let (groups, stats) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
            (RiskLevel::Safe, "test".to_string())
        })
        .unwrap();

        assert_eq!(groups.len(), 1);
        // Stage 4 computed full hash directly, so Stage 5 reread should be 0!
        assert_eq!(stats.full_hash_bytes_read, 0);
        assert_eq!(stats.sample_bytes_read, 200 * 1024);
    }

    #[test]
    fn test_hash_worker_pool_concurrency() {
        PEAK_HASH_WORKERS.store(0, Ordering::SeqCst);
        let dir = tempdir().unwrap();
        let mut entries = Vec::new();
        let content = vec![0x99; 250 * 1024];

        for i in 0..10 {
            let path = dir.path().join(format!("file_{}.bin", i));
            std::fs::write(&path, &content).unwrap();
            entries.push(ScannedEntry::from_path(path).unwrap());
        }

        let options = DuplicateScanOptions {
            min_size: 10 * 1024,
            include_empty: false,
            hash_jobs: 4,
            skip_cloud: true,
            use_cache: false,
        };

        let (groups, _) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
            (RiskLevel::Safe, "test".to_string())
        })
        .unwrap();

        assert_eq!(groups.len(), 1);
        let peak = PEAK_HASH_WORKERS.load(Ordering::SeqCst);
        assert!(peak <= 4, "Peak workers {} must not exceed 4", peak);
        assert!(peak >= 1, "Peak workers {} must be at least 1", peak);
    }

    #[test]
    fn test_error_tolerance_broken_file_does_not_abort_scan() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("good1.bin");
        let file2 = dir.path().join("good2.bin");
        let broken = dir.path().join("broken.bin");

        let content = vec![0x77; 200 * 1024];
        std::fs::write(&file1, &content).unwrap();
        std::fs::write(&file2, &content).unwrap();
        std::fs::write(&broken, &content).unwrap();

        let entry_broken = ScannedEntry::from_path(broken.clone()).unwrap();
        // Remove file to cause I/O error during read
        std::fs::remove_file(&broken).unwrap();

        let entries = vec![
            ScannedEntry::from_path(file1).unwrap(),
            ScannedEntry::from_path(file2).unwrap(),
            entry_broken,
        ];

        let options = DuplicateScanOptions {
            min_size: 10 * 1024,
            include_empty: false,
            hash_jobs: 2,
            skip_cloud: true,
            use_cache: false,
        };

        let (groups, stats) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
            (RiskLevel::Safe, "test".to_string())
        })
        .unwrap();

        // Must still discover the valid duplicate pair!
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members.len(), 2);
        assert!(stats.io_errors_skipped >= 1);
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
