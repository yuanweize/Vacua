use crate::error::{Result, TreeError};
use crate::id::{path_to_raw_bytes, StorageNodeId};
use crate::model::{StorageNodeKind, StorageTreeNode};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use vacua_scan::ScannedEntry;

/// A raw entry representation used to construct the storage tree.
#[derive(Debug, Clone)]
pub struct RawTreeEntry {
    pub relative_path: PathBuf,
    pub is_dir: bool,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub device_id: u64,
    pub inode: u64,
    pub nlink: u64,
    pub mtime_sec: i64,
}

impl From<&ScannedEntry> for RawTreeEntry {
    fn from(s: &ScannedEntry) -> Self {
        Self {
            relative_path: s.path.clone(), // will be re-based to relative by builder
            is_dir: s.is_dir,
            logical_bytes: s.logical_bytes,
            allocated_bytes: s.allocated_bytes,
            device_id: s.device_id,
            inode: s.inode,
            nlink: s.nlink,
            mtime_sec: s.mtime_sec,
        }
    }
}

impl From<&vacua_index::IndexedEntry> for RawTreeEntry {
    fn from(e: &vacua_index::IndexedEntry) -> Self {
        Self {
            relative_path: e.canonical_path.clone(),
            is_dir: e.file_type == "directory",
            logical_bytes: e.logical_bytes,
            allocated_bytes: e.allocated_bytes,
            device_id: e.device_id,
            inode: e.inode,
            nlink: 1,
            mtime_sec: e.mtime_sec,
        }
    }
}

pub struct StorageTreeBuilder<'a> {
    root_path: &'a Path,
    root_id: &'a str,
}

impl<'a> StorageTreeBuilder<'a> {
    pub fn new(root_path: &'a Path, root_id: &'a str) -> Self {
        Self { root_path, root_id }
    }

    /// Builds a deterministic, hardlink-aware hierarchy of StorageTreeNodes.
    pub fn build_from_raw_entries(
        &self,
        mut entries: Vec<RawTreeEntry>,
    ) -> Result<Vec<StorageTreeNode>> {
        // Step 1: Normalize all entry paths relative to root_path
        for entry in &mut entries {
            if entry.relative_path.is_absolute() {
                if let Ok(rel) = entry.relative_path.strip_prefix(self.root_path) {
                    entry.relative_path = rel.to_path_buf();
                }
            }
        }

        // Step 2: Deterministic Hardlink Attribution
        // Files sharing (device_id, inode) attribute their allocated bytes
        // exclusively to the lexicographically smallest canonical raw relative path.
        let mut hardlink_groups: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
        for (idx, entry) in entries.iter().enumerate() {
            if !entry.is_dir && (entry.nlink > 1 || (entry.inode > 0 && entry.device_id > 0)) {
                hardlink_groups
                    .entry((entry.device_id, entry.inode))
                    .or_default()
                    .push(idx);
            }
        }

        // Mark primary vs aliases
        let mut is_alias_map: HashMap<usize, bool> = HashMap::new();
        let mut alias_count_map: HashMap<usize, u64> = HashMap::new();

        for (_key, indices) in hardlink_groups {
            if indices.len() <= 1 {
                continue;
            }
            // Find representative with lexicographically smallest raw relative path
            let mut sorted_indices = indices;
            sorted_indices.sort_by(|&a, &b| {
                let bytes_a = path_to_raw_bytes(&entries[a].relative_path);
                let bytes_b = path_to_raw_bytes(&entries[b].relative_path);
                bytes_a.cmp(&bytes_b)
            });

            let primary_idx = sorted_indices[0];
            let total_aliases = (sorted_indices.len() - 1) as u64;
            alias_count_map.insert(primary_idx, total_aliases);

            for &alias_idx in &sorted_indices[1..] {
                is_alias_map.insert(alias_idx, true);
                // Zero out attributed allocated bytes for aliases
                entries[alias_idx].allocated_bytes = 0;
            }
        }

        // Step 3: Populate nodes table and ensure all ancestor directory nodes exist
        // Map keyed by raw relative path bytes to avoid any lossy conversions
        let mut nodes_by_path: BTreeMap<PathBuf, StorageTreeNode> = BTreeMap::new();

        // Always ensure root node exists (relative path: "")
        let root_node_id = StorageNodeId::from_raw_relative(self.root_id, b"");
        let root_display_name = self
            .root_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.root_path.to_string_lossy().to_string());

        let root_node = StorageTreeNode {
            node_id: root_node_id,
            parent_id: None,
            raw_relative_path: Vec::new(),
            display_name: root_display_name,
            display_path: self.root_path.to_string_lossy().to_string(),
            kind: StorageNodeKind::Root,
            depth: 0,
            direct_logical_bytes: 0,
            direct_allocated_bytes: 0,
            subtree_logical_bytes: 0,
            subtree_allocated_bytes: 0,
            file_count: 0,
            directory_count: 0,
            hardlink_alias_count: 0,
            is_hardlink_alias: false,
            child_count: 0,
            mtime_sec: 0,
        };
        nodes_by_path.insert(PathBuf::new(), root_node);

        // Process entries
        for (idx, entry) in entries.into_iter().enumerate() {
            let rel_path = entry.relative_path;
            if rel_path.as_os_str().is_empty() {
                // This is the root itself
                if let Some(r) = nodes_by_path.get_mut(&PathBuf::new()) {
                    r.mtime_sec = entry.mtime_sec;
                }
                continue;
            }

            // Ensure all ancestor directories exist
            let mut current = PathBuf::new();
            let mut parent_id = StorageNodeId::from_raw_relative(self.root_id, b"");

            let components: Vec<_> = rel_path.components().collect();
            let is_last_idx = components.len() - 1;

            for (c_idx, comp) in components.iter().enumerate() {
                let depth = (c_idx + 1) as u32;
                current.push(comp);
                let is_leaf = c_idx == is_last_idx;
                let raw_bytes = path_to_raw_bytes(&current);
                let current_node_id = StorageNodeId::from_raw_relative(self.root_id, &raw_bytes);

                if is_leaf {
                    let display_name = comp.as_os_str().to_string_lossy().to_string();
                    let display_path = self.root_path.join(&current).to_string_lossy().to_string();
                    let is_alias = is_alias_map.get(&idx).copied().unwrap_or(false);
                    let alias_count = alias_count_map.get(&idx).copied().unwrap_or(0);

                    let node = StorageTreeNode {
                        node_id: current_node_id.clone(),
                        parent_id: Some(parent_id),
                        raw_relative_path: raw_bytes,
                        display_name,
                        display_path,
                        kind: if entry.is_dir {
                            StorageNodeKind::Directory
                        } else {
                            StorageNodeKind::File
                        },
                        depth,
                        direct_logical_bytes: entry.logical_bytes,
                        direct_allocated_bytes: entry.allocated_bytes,
                        subtree_logical_bytes: entry.logical_bytes,
                        subtree_allocated_bytes: entry.allocated_bytes,
                        file_count: if entry.is_dir { 0 } else { 1 },
                        directory_count: 0,
                        hardlink_alias_count: alias_count,
                        is_hardlink_alias: is_alias,
                        child_count: 0,
                        mtime_sec: entry.mtime_sec,
                    };
                    nodes_by_path.insert(current.clone(), node);
                } else if !nodes_by_path.contains_key(&current) {
                    // Create intermediate directory node
                    let display_name = comp.as_os_str().to_string_lossy().to_string();
                    let display_path = self.root_path.join(&current).to_string_lossy().to_string();

                    let dir_node = StorageTreeNode {
                        node_id: current_node_id.clone(),
                        parent_id: Some(parent_id),
                        raw_relative_path: raw_bytes,
                        display_name,
                        display_path,
                        kind: StorageNodeKind::Directory,
                        depth,
                        direct_logical_bytes: 0,
                        direct_allocated_bytes: 0,
                        subtree_logical_bytes: 0,
                        subtree_allocated_bytes: 0,
                        file_count: 0,
                        directory_count: 0,
                        hardlink_alias_count: 0,
                        is_hardlink_alias: false,
                        child_count: 0,
                        mtime_sec: 0,
                    };
                    nodes_by_path.insert(current.clone(), dir_node);
                }

                parent_id = current_node_id;
            }
        }

        // Step 4: Iterative Bottom-Up Rollup (depth-descending pass)
        // Group paths by depth to roll up metrics from leaves to root without stack recursion
        let mut paths_by_depth: BTreeMap<u32, Vec<PathBuf>> = BTreeMap::new();
        for (path, node) in &nodes_by_path {
            paths_by_depth
                .entry(node.depth)
                .or_default()
                .push(path.clone());
        }

        let max_depth = paths_by_depth.keys().copied().max().unwrap_or(0);
        for d in (1..=max_depth).rev() {
            if let Some(paths) = paths_by_depth.get(&d) {
                for path in paths {
                    let child = match nodes_by_path.get(path) {
                        Some(c) => c.clone(),
                        None => continue,
                    };

                    let parent_path = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                    if let Some(parent) = nodes_by_path.get_mut(&parent_path) {
                        parent.subtree_logical_bytes = parent
                            .subtree_logical_bytes
                            .checked_add(child.subtree_logical_bytes)
                            .ok_or(TreeError::CounterOverflow)?;

                        parent.subtree_allocated_bytes = parent
                            .subtree_allocated_bytes
                            .checked_add(child.subtree_allocated_bytes)
                            .ok_or(TreeError::CounterOverflow)?;

                        parent.child_count = parent
                            .child_count
                            .checked_add(1)
                            .ok_or(TreeError::CounterOverflow)?;

                        if child.kind == StorageNodeKind::Directory {
                            parent.directory_count = parent
                                .directory_count
                                .checked_add(1 + child.directory_count)
                                .ok_or(TreeError::CounterOverflow)?;
                            parent.file_count = parent
                                .file_count
                                .checked_add(child.file_count)
                                .ok_or(TreeError::CounterOverflow)?;
                            parent.hardlink_alias_count = parent
                                .hardlink_alias_count
                                .checked_add(child.hardlink_alias_count)
                                .ok_or(TreeError::CounterOverflow)?;
                        } else {
                            parent.file_count = parent
                                .file_count
                                .checked_add(1)
                                .ok_or(TreeError::CounterOverflow)?;
                            if child.is_hardlink_alias {
                                parent.hardlink_alias_count = parent
                                    .hardlink_alias_count
                                    .checked_add(1)
                                    .ok_or(TreeError::CounterOverflow)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(nodes_by_path.into_values().collect())
    }

    /// Convenience helper to build from ScannedEntry slice.
    pub fn build_from_scanned_entries(
        &self,
        entries: &[ScannedEntry],
    ) -> Result<Vec<StorageTreeNode>> {
        let raw_entries: Vec<RawTreeEntry> = entries.iter().map(RawTreeEntry::from).collect();
        self.build_from_raw_entries(raw_entries)
    }

    /// Convenience helper to build from IndexedEntry slice.
    pub fn build_from_indexed_entries(
        &self,
        entries: &[vacua_index::IndexedEntry],
    ) -> Result<Vec<StorageTreeNode>> {
        let raw_entries: Vec<RawTreeEntry> = entries.iter().map(RawTreeEntry::from).collect();
        self.build_from_raw_entries(raw_entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_file_tree() {
        let root = Path::new("/tmp/test_root");
        let builder = StorageTreeBuilder::new(root, "root_1");

        let entries = vec![RawTreeEntry {
            relative_path: PathBuf::from("file.txt"),
            is_dir: false,
            logical_bytes: 100,
            allocated_bytes: 512,
            device_id: 1,
            inode: 10,
            nlink: 1,
            mtime_sec: 1000,
        }];

        let nodes = builder.build_from_raw_entries(entries).unwrap();
        assert_eq!(nodes.len(), 2); // root + file.txt

        let root_node = nodes
            .iter()
            .find(|n| n.kind == StorageNodeKind::Root)
            .unwrap();
        assert_eq!(root_node.subtree_logical_bytes, 100);
        assert_eq!(root_node.subtree_allocated_bytes, 512);
        assert_eq!(root_node.file_count, 1);
        assert_eq!(root_node.directory_count, 0);
        assert_eq!(root_node.child_count, 1);
    }

    #[test]
    fn test_hardlink_deduplication_allocated_not_doubled() {
        let root = Path::new("/tmp/test_root");
        let builder = StorageTreeBuilder::new(root, "root_1");

        // Two hardlinks sharing device 1, inode 42, nlink 2
        let entries = vec![
            RawTreeEntry {
                relative_path: PathBuf::from("dir_b/link_b.bin"),
                is_dir: false,
                logical_bytes: 1024,
                allocated_bytes: 4096,
                device_id: 1,
                inode: 42,
                nlink: 2,
                mtime_sec: 1000,
            },
            RawTreeEntry {
                relative_path: PathBuf::from("dir_a/link_a.bin"),
                is_dir: false,
                logical_bytes: 1024,
                allocated_bytes: 4096,
                device_id: 1,
                inode: 42,
                nlink: 2,
                mtime_sec: 1000,
            },
        ];

        let nodes = builder.build_from_raw_entries(entries).unwrap();
        let root_node = nodes
            .iter()
            .find(|n| n.kind == StorageNodeKind::Root)
            .unwrap();

        // Logical bytes: 1024 + 1024 = 2048
        assert_eq!(root_node.subtree_logical_bytes, 2048);
        // Allocated bytes: 4096 only (NOT 8192)
        assert_eq!(root_node.subtree_allocated_bytes, 4096);
        assert_eq!(root_node.file_count, 2);
        assert_eq!(root_node.hardlink_alias_count, 1);

        // Check primary representative: dir_a/link_a.bin (lexicographically smaller than dir_b/link_b.bin)
        let link_a = nodes
            .iter()
            .find(|n| n.display_name == "link_a.bin")
            .unwrap();
        assert_eq!(link_a.direct_allocated_bytes, 4096);
        assert!(!link_a.is_hardlink_alias);
        assert_eq!(link_a.hardlink_alias_count, 1);

        let link_b = nodes
            .iter()
            .find(|n| n.display_name == "link_b.bin")
            .unwrap();
        assert_eq!(link_b.direct_allocated_bytes, 0);
        assert!(link_b.is_hardlink_alias);
    }

    #[test]
    fn test_deep_hierarchy_safety() {
        let root = Path::new("/tmp/test_root");
        let builder = StorageTreeBuilder::new(root, "root_1");

        // Build path of depth 256
        let mut deep_path = PathBuf::new();
        for i in 0..256 {
            deep_path.push(format!("d_{:03}", i));
        }
        deep_path.push("leaf.txt");

        let entries = vec![RawTreeEntry {
            relative_path: deep_path,
            is_dir: false,
            logical_bytes: 50,
            allocated_bytes: 512,
            device_id: 1,
            inode: 999,
            nlink: 1,
            mtime_sec: 2000,
        }];

        let nodes = builder.build_from_raw_entries(entries).unwrap();
        let root_node = nodes
            .iter()
            .find(|n| n.kind == StorageNodeKind::Root)
            .unwrap();
        assert_eq!(root_node.subtree_logical_bytes, 50);
        assert_eq!(root_node.subtree_allocated_bytes, 512);
        assert_eq!(root_node.file_count, 1);
        assert_eq!(root_node.directory_count, 256);
    }

    #[test]
    fn test_counter_overflow_detection() {
        let root = Path::new("/tmp/test_root");
        let builder = StorageTreeBuilder::new(root, "root_1");

        let entries = vec![
            RawTreeEntry {
                relative_path: PathBuf::from("a.bin"),
                is_dir: false,
                logical_bytes: u64::MAX - 10,
                allocated_bytes: 100,
                device_id: 1,
                inode: 1,
                nlink: 1,
                mtime_sec: 1,
            },
            RawTreeEntry {
                relative_path: PathBuf::from("b.bin"),
                is_dir: false,
                logical_bytes: 20,
                allocated_bytes: 100,
                device_id: 1,
                inode: 2,
                nlink: 1,
                mtime_sec: 1,
            },
        ];

        let err = builder.build_from_raw_entries(entries).unwrap_err();
        match err {
            TreeError::CounterOverflow => {}
            _ => panic!("Expected CounterOverflow error, got {:?}", err),
        }
    }
}
