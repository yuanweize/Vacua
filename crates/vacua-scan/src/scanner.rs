use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread;
use walkdir::WalkDir;

use crate::entry::ScannedEntry;
use vacua_core::allocation::AllocationInfo;

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub cross_mounts: bool,
    pub max_depth: Option<usize>,
    pub jobs: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ScanReport {
    pub root_path: PathBuf,
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_symlinks: u64,
    pub sparse_files: u64,
    pub clone_files: u64,
    pub allocation: AllocationInfo,
    pub skipped_paths: Vec<String>,
    pub entries: Vec<ScannedEntry>,
}

pub struct FilesystemScanner {
    options: ScanOptions,
}

impl FilesystemScanner {
    pub fn new(options: ScanOptions) -> Self {
        Self { options }
    }

    /// Run streaming, bounded-concurrency filesystem traversal starting from `root`.
    /// Guaranteed to:
    /// 1. Never follow symlinks (prevents recursive cycles and target mutations).
    /// 2. Deduplicate hard links via (device_id, inode) sets so physical allocated space is not double-counted.
    /// 3. Track APFS clone families via clone_id and clone_refcnt for true physical storage attribution.
    /// 4. Run bounded parallel stat / metadata workers with backpressure.
    /// 5. Deterministically sort output entries by path.
    pub fn scan(&self, root: &Path) -> std::io::Result<ScanReport> {
        let concurrency = self.options.jobs.unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .clamp(1, 16)
        });

        if concurrency <= 1 {
            self.scan_sequential(root)
        } else {
            self.scan_bounded_concurrent(root, concurrency)
        }
    }

    fn scan_sequential(&self, root: &Path) -> std::io::Result<ScanReport> {
        let mut walker = WalkDir::new(root).follow_links(false);
        if !self.options.cross_mounts {
            walker = walker.same_file_system(true);
        }
        if let Some(depth) = self.options.max_depth {
            walker = walker.max_depth(depth);
        }

        let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
        let mut seen_clone_ids: HashSet<u64> = HashSet::new();
        let mut entries = Vec::new();
        let mut skipped_paths = Vec::new();

        let mut total_files = 0;
        let mut total_dirs = 0;
        let mut total_symlinks = 0;
        let mut sparse_files = 0;
        let mut clone_files = 0;

        let mut total_logical = 0u64;
        let mut total_allocated = 0u64;
        let mut total_shared = 0u64;
        let mut total_exclusive = 0u64;

        for entry_res in walker {
            match entry_res {
                Ok(dir_entry) => {
                    let path = dir_entry.path().to_path_buf();
                    match ScannedEntry::from_path(path.clone()) {
                        Ok(scanned) => {
                            if scanned.is_symlink {
                                total_symlinks += 1;
                            } else if scanned.is_dir {
                                total_dirs += 1;
                            } else {
                                total_files += 1;
                            }

                            if scanned.is_sparse {
                                sparse_files += 1;
                            }

                            if scanned.is_clone {
                                clone_files += 1;
                            }

                            // Deduplicate physical blocks by inode (hardlinks)
                            let inode_key = (scanned.device_id, scanned.inode);
                            let is_first_inode = seen_inodes.insert(inode_key);

                            if is_first_inode {
                                total_allocated =
                                    total_allocated.saturating_add(scanned.allocated_bytes);

                                // APFS clone family accounting
                                if let Some(cid) = scanned.clone_id {
                                    if scanned.is_clone {
                                        if seen_clone_ids.insert(cid) {
                                            // First clone seen in this family: counts as shared
                                            total_shared = total_shared
                                                .saturating_add(scanned.allocated_bytes);
                                        } else {
                                            // Duplicate clone in same family: physical blocks already counted
                                        }
                                    } else {
                                        total_exclusive =
                                            total_exclusive.saturating_add(scanned.allocated_bytes);
                                    }
                                } else {
                                    total_exclusive =
                                        total_exclusive.saturating_add(scanned.allocated_bytes);
                                }
                            }

                            if !scanned.is_dir && !scanned.is_symlink {
                                total_logical = total_logical.saturating_add(scanned.logical_bytes);
                            }
                            entries.push(scanned);
                        }
                        Err(err) => {
                            skipped_paths.push(format!("{}: {}", path.display(), err));
                        }
                    }
                }
                Err(err) => {
                    let path_str = err
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    skipped_paths.push(format!("{}: {}", path_str, err));
                }
            }
        }

        // Deterministic sorting by path
        entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));

        let mut allocation = AllocationInfo::new(total_logical, total_allocated, clone_files > 0);
        allocation.shared_bytes = total_shared;
        allocation.exclusive_bytes = total_exclusive;
        allocation.is_clone = clone_files > 0;

        Ok(ScanReport {
            root_path: root.to_path_buf(),
            total_files,
            total_dirs,
            total_symlinks,
            sparse_files,
            clone_files,
            allocation,
            skipped_paths,
            entries,
        })
    }

    fn scan_bounded_concurrent(&self, root: &Path, jobs: usize) -> std::io::Result<ScanReport> {
        let channel_bound = 2048;
        let (tx, rx): (SyncSender<Option<PathBuf>>, Receiver<Option<PathBuf>>) =
            sync_channel(channel_bound);
        let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
        let (res_tx, res_rx) = std::sync::mpsc::channel();

        // Spawn worker pool
        let mut handles = Vec::with_capacity(jobs);
        for _ in 0..jobs {
            let worker_rx = std::sync::Arc::clone(&rx);
            let worker_tx = res_tx.clone();
            let handle = thread::spawn(move || loop {
                let path_opt = {
                    let Ok(guard) = worker_rx.lock() else { break };
                    guard.recv().ok().flatten()
                };

                match path_opt {
                    Some(path) => {
                        let res = ScannedEntry::from_path(path);
                        if worker_tx.send(res).is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            });
            handles.push(handle);
        }
        drop(res_tx); // Drop extra sender so receiver knows when work finishes

        // Producer: Walk directory and send paths into bounded channel with backpressure
        let mut walker = WalkDir::new(root).follow_links(false);
        if !self.options.cross_mounts {
            walker = walker.same_file_system(true);
        }
        if let Some(depth) = self.options.max_depth {
            walker = walker.max_depth(depth);
        }

        let mut skipped_paths = Vec::new();
        for entry_res in walker {
            match entry_res {
                Ok(dir_entry) => {
                    let path = dir_entry.path().to_path_buf();
                    if tx.send(Some(path)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let path_str = err
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    skipped_paths.push(format!("{}: {}", path_str, err));
                }
            }
        }

        // Signal workers to terminate
        for _ in 0..jobs {
            let _ = tx.send(None);
        }

        // Collector: Aggregate results
        let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
        let mut seen_clone_ids: HashSet<u64> = HashSet::new();
        let mut entries = Vec::new();

        let mut total_files = 0;
        let mut total_dirs = 0;
        let mut total_symlinks = 0;
        let mut sparse_files = 0;
        let mut clone_files = 0;

        let mut total_logical = 0u64;
        let mut total_allocated = 0u64;
        let mut total_shared = 0u64;
        let mut total_exclusive = 0u64;

        while let Ok(result) = res_rx.recv() {
            match result {
                Ok(scanned) => {
                    if scanned.is_symlink {
                        total_symlinks += 1;
                    } else if scanned.is_dir {
                        total_dirs += 1;
                    } else {
                        total_files += 1;
                    }

                    if scanned.is_sparse {
                        sparse_files += 1;
                    }

                    if scanned.is_clone {
                        clone_files += 1;
                    }

                    let inode_key = (scanned.device_id, scanned.inode);
                    let is_first_inode = seen_inodes.insert(inode_key);

                    if is_first_inode {
                        total_allocated = total_allocated.saturating_add(scanned.allocated_bytes);

                        if let Some(cid) = scanned.clone_id {
                            if scanned.is_clone {
                                if seen_clone_ids.insert(cid) {
                                    total_shared =
                                        total_shared.saturating_add(scanned.allocated_bytes);
                                }
                            } else {
                                total_exclusive =
                                    total_exclusive.saturating_add(scanned.allocated_bytes);
                            }
                        } else {
                            total_exclusive =
                                total_exclusive.saturating_add(scanned.allocated_bytes);
                        }
                    }

                    if !scanned.is_dir && !scanned.is_symlink {
                        total_logical = total_logical.saturating_add(scanned.logical_bytes);
                    }
                    entries.push(scanned);
                }
                Err(err) => {
                    skipped_paths.push(format!("Worker error: {}", err));
                }
            }
        }

        // Wait for workers
        for h in handles {
            let _ = h.join();
        }

        // Deterministic sorting by path invariant
        entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));

        let mut allocation = AllocationInfo::new(total_logical, total_allocated, clone_files > 0);
        allocation.shared_bytes = total_shared;
        allocation.exclusive_bytes = total_exclusive;
        allocation.is_clone = clone_files > 0;

        Ok(ScanReport {
            root_path: root.to_path_buf(),
            total_files,
            total_dirs,
            total_symlinks,
            sparse_files,
            clone_files,
            allocation,
            skipped_paths,
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_scanner_deduplicates_hardlinks() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("file1.bin");
        let file2 = dir.path().join("file2.bin");

        let data = vec![0x42u8; 4096];
        let mut f = File::create(&file1).unwrap();
        f.write_all(&data).unwrap();
        drop(f);

        fs::hard_link(&file1, &file2).unwrap();

        let scanner = FilesystemScanner::new(ScanOptions::default());
        let report = scanner.scan(dir.path()).unwrap();

        assert_eq!(report.total_files, 2);
        assert_eq!(report.allocation.logical_bytes, 8192);
        let file1_entry = report.entries.iter().find(|e| e.path == file1).unwrap();
        assert_eq!(
            report.allocation.allocated_bytes,
            file1_entry.allocated_bytes
        );
    }

    #[test]
    fn test_scanner_bounded_concurrent_matches_sequential() {
        let dir = tempdir().unwrap();
        for i in 0..50 {
            let p = dir.path().join(format!("file_{}.txt", i));
            fs::write(&p, format!("content {}", i)).unwrap();
        }

        let seq_scanner = FilesystemScanner::new(ScanOptions {
            jobs: Some(1),
            ..Default::default()
        });
        let conc_scanner = FilesystemScanner::new(ScanOptions {
            jobs: Some(4),
            ..Default::default()
        });

        let seq_report = seq_scanner.scan(dir.path()).unwrap();
        let conc_report = conc_scanner.scan(dir.path()).unwrap();

        assert_eq!(seq_report.total_files, conc_report.total_files);
        assert_eq!(
            seq_report.allocation.logical_bytes,
            conc_report.allocation.logical_bytes
        );
        assert_eq!(
            seq_report.allocation.allocated_bytes,
            conc_report.allocation.allocated_bytes
        );

        // Invariant: Paths must be sorted in exact same deterministic order
        assert_eq!(seq_report.entries.len(), conc_report.entries.len());
        for (a, b) in seq_report.entries.iter().zip(conc_report.entries.iter()) {
            assert_eq!(a.path, b.path);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_real_apfs_clonefile_detection() {
        use std::ffi::CString;

        extern "C" {
            fn clonefile(
                src: *const std::ffi::c_char,
                dst: *const std::ffi::c_char,
                flags: u32,
            ) -> i32;
        }

        let dir = tempdir().unwrap();
        let orig = dir.path().join("original.dat");
        let clone = dir.path().join("cloned.dat");

        let data = vec![0xAAu8; 16384];
        fs::write(&orig, &data).unwrap();

        let c_orig = CString::new(orig.to_str().unwrap()).unwrap();
        let c_clone = CString::new(clone.to_str().unwrap()).unwrap();

        let res = unsafe { clonefile(c_orig.as_ptr(), c_clone.as_ptr(), 0) };
        if res == 0 {
            let scanner = FilesystemScanner::new(ScanOptions::default());
            let report = scanner.scan(dir.path()).unwrap();

            assert_eq!(report.total_files, 2);
            assert!(report.clone_files >= 1);
            assert!(report.allocation.is_clone);
            assert!(report.allocation.shared_bytes > 0);
        }
    }

    #[test]
    fn test_scanner_does_not_follow_symlink_into_foreign_dir() {
        let dir = tempdir().unwrap();
        let outside_dir = tempdir().unwrap();
        let outside_file = outside_dir.path().join("secret.txt");
        fs::write(&outside_file, b"secret").unwrap();

        let link_inside = dir.path().join("link_to_outside");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside_dir.path(), &link_inside).unwrap();

        let scanner = FilesystemScanner::new(ScanOptions::default());
        let report = scanner.scan(dir.path()).unwrap();

        assert_eq!(report.total_symlinks, 1);
        assert_eq!(report.total_files, 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_apfs_private_size_semantics() {
        use std::ffi::CString;

        extern "C" {
            fn clonefile(
                src: *const std::ffi::c_char,
                dst: *const std::ffi::c_char,
                flags: u32,
            ) -> i32;
        }

        let dir = tempdir().unwrap();
        let orig = dir.path().join("orig.bin");
        let clone = dir.path().join("clone.bin");
        let mutated = dir.path().join("mutated.bin");

        let data = vec![0xBB; 64 * 1024]; // 64 KiB
        fs::write(&orig, &data).unwrap();

        let c_orig = CString::new(orig.to_str().unwrap()).unwrap();
        let c_clone = CString::new(clone.to_str().unwrap()).unwrap();
        let c_mutated = CString::new(mutated.to_str().unwrap()).unwrap();

        if unsafe { clonefile(c_orig.as_ptr(), c_clone.as_ptr(), 0) } == 0
            && unsafe { clonefile(c_orig.as_ptr(), c_mutated.as_ptr(), 0) } == 0
        {
            // Mutate 'mutated.bin' with 4 KiB of distinct data
            use std::io::{Seek, SeekFrom, Write};
            let mut f = fs::OpenOptions::new().write(true).open(&mutated).unwrap();
            f.seek(SeekFrom::Start(0)).unwrap();
            f.write_all(&vec![0xCC; 4096]).unwrap();
            f.flush().unwrap();
            drop(f);

            let entry_clone = ScannedEntry::from_path(clone).unwrap();
            let entry_mutated = ScannedEntry::from_path(mutated).unwrap();

            // If private size was retrieved from APFS kernel:
            if let Some(priv_clone) = entry_clone.kernel_private_bytes {
                // Invariant: Unmodified clonefile shares all extents, private size is 0!
                assert_eq!(priv_clone, 0, "Unmodified clonefile private size must be 0");
            }
            if let (Some(priv_mut), Some(priv_clone)) = (
                entry_mutated.kernel_private_bytes,
                entry_clone.kernel_private_bytes,
            ) {
                // Invariant: Mutated clone has private size > unmodified clone
                assert!(
                    priv_mut > priv_clone,
                    "Mutated clone private size ({}) must exceed clone ({})",
                    priv_mut,
                    priv_clone
                );
            }
        }
    }
}
