use crate::id::StorageNodeId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageNodeKind {
    Root,
    Directory,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageTreeMetric {
    #[default]
    Allocated,
    Logical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageTreeSource {
    Indexed,
    LiveScan,
    IncrementalRefresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageTreeStatus {
    Building,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageTreeCoverage {
    pub files_observed: u64,
    pub directories_observed: u64,
    pub entries_skipped: u64,
    pub permission_errors: u64,
    pub mount_boundary_skips: u64,
    pub special_files_skipped: u64,
    pub cloud_placeholders_observed: u64,
    pub analysis_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageTreeNode {
    pub node_id: StorageNodeId,
    pub parent_id: Option<StorageNodeId>,
    pub raw_relative_path: Vec<u8>,
    pub display_name: String,
    pub display_path: String,
    pub kind: StorageNodeKind,
    pub depth: u32,
    pub direct_logical_bytes: u64,
    pub direct_allocated_bytes: u64,
    pub subtree_logical_bytes: u64,
    pub subtree_allocated_bytes: u64,
    pub file_count: u64,
    pub directory_count: u64,
    pub hardlink_alias_count: u64,
    pub is_hardlink_alias: bool,
    pub child_count: u64,
    pub mtime_sec: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageTreeRemainder {
    pub item_count: u64,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageTreeGeneration {
    pub generation_id: String,
    pub root_path: PathBuf,
    pub root_id: String,
    pub observed_at: i64,
    pub source: StorageTreeSource,
    pub status: StorageTreeStatus,
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_logical_bytes: u64,
    pub total_allocated_bytes: u64,
    pub coverage: StorageTreeCoverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeChangeKind {
    New,
    Grown,
    Shrunk,
    Unchanged,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageTreeDelta {
    pub node_id: StorageNodeId,
    pub allocated_delta_bytes: i64,
    pub logical_delta_bytes: i64,
    pub file_count_delta: i64,
    pub change_kind: TreeChangeKind,
}

impl StorageTreeNode {
    pub fn to_dto(&self) -> vacua_api::StorageTreeNodeV1 {
        vacua_api::StorageTreeNodeV1 {
            node_id: self.node_id.to_string(),
            parent_id: self.parent_id.as_ref().map(|p| p.to_string()),
            display_name: self.display_name.clone(),
            display_path: self.display_path.clone(),
            kind: match self.kind {
                StorageNodeKind::Root => "root".to_string(),
                StorageNodeKind::Directory => "directory".to_string(),
                StorageNodeKind::File => "file".to_string(),
            },
            depth: self.depth,
            direct_logical_bytes: self.direct_logical_bytes,
            direct_allocated_bytes: self.direct_allocated_bytes,
            subtree_logical_bytes: self.subtree_logical_bytes,
            subtree_allocated_bytes: self.subtree_allocated_bytes,
            file_count: self.file_count,
            directory_count: self.directory_count,
            hardlink_alias_count: self.hardlink_alias_count,
            is_hardlink_alias: self.is_hardlink_alias,
            child_count: self.child_count,
            mtime_sec: self.mtime_sec,
        }
    }
}

impl StorageTreeRemainder {
    pub fn to_dto(&self) -> vacua_api::StorageTreeRemainderV1 {
        vacua_api::StorageTreeRemainderV1 {
            item_count: self.item_count,
            logical_bytes: self.logical_bytes,
            allocated_bytes: self.allocated_bytes,
        }
    }
}

impl StorageTreeCoverage {
    pub fn to_dto(&self) -> vacua_api::StorageTreeCoverageV1 {
        vacua_api::StorageTreeCoverageV1 {
            files_observed: self.files_observed,
            directories_observed: self.directories_observed,
            entries_skipped: self.entries_skipped,
            permission_errors: self.permission_errors,
            mount_boundary_skips: self.mount_boundary_skips,
            special_files_skipped: self.special_files_skipped,
            cloud_placeholders_observed: self.cloud_placeholders_observed,
            analysis_complete: self.analysis_complete,
        }
    }
}

impl StorageTreeDelta {
    pub fn to_dto(&self) -> vacua_api::StorageTreeDeltaV1 {
        vacua_api::StorageTreeDeltaV1 {
            node_id: self.node_id.to_string(),
            allocated_delta_bytes: self.allocated_delta_bytes,
            logical_delta_bytes: self.logical_delta_bytes,
            file_count_delta: self.file_count_delta,
            change_kind: match self.change_kind {
                TreeChangeKind::New => "new".to_string(),
                TreeChangeKind::Grown => "grown".to_string(),
                TreeChangeKind::Shrunk => "shrunk".to_string(),
                TreeChangeKind::Unchanged => "unchanged".to_string(),
                TreeChangeKind::Removed => "removed".to_string(),
            },
        }
    }
}

impl StorageTreeGeneration {
    pub fn to_dto(
        &self,
        root_node: vacua_api::StorageTreeNodeV1,
    ) -> vacua_api::StorageTreeAnalysisV1 {
        let observed_at_str = chrono::DateTime::from_timestamp(self.observed_at, 0)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| self.observed_at.to_string());

        vacua_api::StorageTreeAnalysisV1 {
            schema_version: vacua_api::SCHEMA_STORAGE_TREE_ANALYSIS_V1.to_string(),
            generation_id: self.generation_id.clone(),
            root_path: self.root_path.to_string_lossy().to_string(),
            root_id: self.root_id.clone(),
            observed_at: observed_at_str,
            source: match self.source {
                StorageTreeSource::LiveScan => "LiveScan".to_string(),
                StorageTreeSource::IncrementalRefresh => "IncrementalRefresh".to_string(),
                StorageTreeSource::Indexed => "Indexed".to_string(),
            },
            root_node,
            total_files: self.total_files,
            total_dirs: self.total_dirs,
            total_logical_bytes: self.total_logical_bytes,
            total_allocated_bytes: self.total_allocated_bytes,
            physical_sharing_uncertainty: true,
            allocation_semantics: "Filesystem allocation attributed to this namespace tree. APFS clone sharing may cause physical overlap.".to_string(),
            coverage: self.coverage.to_dto(),
        }
    }
}
