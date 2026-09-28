use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::entry::ScannedEntry;
use vacua_core::allocation::AllocationInfo;

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub cross_mounts: bool,
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ScanReport {
    pub root_path: PathBuf,
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_symlinks: u64,
    pub sparse_files: u64,
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

    /// Run streaming filesystem traversal starting from `root`.
    /// Guaranteed to:
    /// 1. Never follow symlinks (prevents recursive cycles and target mutations).
    /// 2. Deduplicate hard links via (device_id, inode) sets so physical allocated space is not double-counted.
    /// 3. Skip permission denied items gracefully without crashing.
    /// 4. Respect mount boundaries unless explicit cross_mounts is enabled.
    pub fn scan(&self, root: &Path) -> std::io::Result<ScanReport> {
        let mut walker = WalkDir::new(root).follow_links(false);

        if !self.options.cross_mounts {
            walker = walker.same_file_system(true);
        }

        if let Some(depth) = self.options.max_depth {
            walker = walker.max_depth(depth);
        }

        let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();
        let mut entries = Vec::new();
        let mut skipped_paths = Vec::new();

        let mut total_files = 0;
        let mut total_dirs = 0;
        let mut total_symlinks = 0;
        let mut sparse_files = 0;

        let mut total_logical = 0u64;
        let mut total_allocated = 0u64;

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

                            // Deduplicate physical blocks by inode
                            let inode_key = (scanned.device_id, scanned.inode);
                            if !seen_inodes.contains(&inode_key) {
                                seen_inodes.insert(inode_key);
                                total_allocated =
                                    total_allocated.saturating_add(scanned.allocated_bytes);
                            }

                            total_logical = total_logical.saturating_add(scanned.logical_bytes);
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

        let allocation = AllocationInfo::new(total_logical, total_allocated, false);

        Ok(ScanReport {
            root_path: root.to_path_buf(),
            total_files,
            total_dirs,
            total_symlinks,
            sparse_files,
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

        // Write 4096 bytes to file1
        let data = vec![0x42u8; 4096];
        let mut f = File::create(&file1).unwrap();
        f.write_all(&data).unwrap();
        drop(f);

        // Create hard link file2 -> file1
        fs::hard_link(&file1, &file2).unwrap();

        let scanner = FilesystemScanner::new(ScanOptions::default());
        let report = scanner.scan(dir.path()).unwrap();

        assert_eq!(report.total_files, 2);
        // Logical size includes directory metadata node (e.g. 128 bytes on APFS) + 8192 bytes from both file entries
        assert!(report.allocation.logical_bytes >= 8192);
        // Allocated physical size should NOT be double counted because of shared inode!
        assert!(
            report.allocation.allocated_bytes < 8192,
            "Allocated bytes ({}) should not double-count hard-linked file",
            report.allocation.allocated_bytes
        );
    }

    #[test]
    fn test_scanner_does_not_follow_symlink_into_foreign_dir() {
        let dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        // Create a secret file in target_dir
        let secret = target_dir.path().join("secret.txt");
        File::create(&secret).unwrap();

        // Create symlink inside dir pointing to target_dir
        let symlink_path = dir.path().join("link_to_target");
        #[cfg(unix)]
        std::os::unix::fs::symlink(target_dir.path(), &symlink_path).unwrap();

        let scanner = FilesystemScanner::new(ScanOptions::default());
        let report = scanner.scan(dir.path()).unwrap();

        // Must find the symlink entry, but must NEVER traverse into target_dir
        assert_eq!(report.total_symlinks, 1);
        let found_secret = report
            .entries
            .iter()
            .any(|e| e.path.ends_with("secret.txt"));
        assert!(
            !found_secret,
            "Scanner must not follow symlinks into external directories"
        );
    }
}
