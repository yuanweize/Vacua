use serde::{Deserialize, Serialize};

/// Detailed allocation metrics distinguishing logical content size from
/// physical blocks allocated on disk (APFS / HFS+ blocks), with full clone awareness.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AllocationInfo {
    /// Nominal byte size of the file contents (st_size).
    pub logical_bytes: u64,

    /// True physical bytes allocated on storage (st_blocks * 512).
    pub allocated_bytes: u64,

    /// Physical bytes shared with other files via APFS clonefile or deduplication.
    pub shared_bytes: u64,

    /// Physical bytes exclusively attributed to this file that will physically free upon deletion.
    pub exclusive_bytes: u64,

    /// Estimated bytes that can be freed if reclaimed.
    pub potentially_reclaimable_bytes: u64,

    /// Whether this file exhibits sparse allocation (allocated < logical).
    pub is_sparse: bool,

    /// Whether this file is part of an APFS clone family.
    pub is_clone: bool,

    /// Unique 64-bit APFS clone family identifier if present.
    pub clone_id: Option<u64>,

    /// Number of clone references in the APFS clone family.
    pub clone_refcnt: u32,

    /// True if physical saving is uncertain due to potential APFS clonefile extent sharing.
    pub extent_uncertainty: bool,

    /// Mathematical confidence in reclaiming physical bytes (0.0 to 1.0).
    pub reclaim_confidence: f32,
}

impl AllocationInfo {
    pub fn new(logical_bytes: u64, allocated_bytes: u64, extent_uncertainty: bool) -> Self {
        let is_sparse = allocated_bytes < logical_bytes;
        let potentially_reclaimable_bytes = allocated_bytes;
        let exclusive_bytes = allocated_bytes;

        Self {
            logical_bytes,
            allocated_bytes,
            shared_bytes: 0,
            exclusive_bytes,
            potentially_reclaimable_bytes,
            is_sparse,
            is_clone: false,
            clone_id: None,
            clone_refcnt: 1,
            extent_uncertainty,
            reclaim_confidence: if extent_uncertainty { 0.5 } else { 1.0 },
        }
    }

    pub fn new_with_clone(
        logical_bytes: u64,
        allocated_bytes: u64,
        clone_id: Option<u64>,
        clone_refcnt: u32,
        is_clone: bool,
        is_sparse: bool,
    ) -> Self {
        let (shared_bytes, exclusive_bytes, reclaim_confidence, extent_uncertainty) = if is_clone {
            // For a clone with multiple references, deleting one reference frees 0 physical blocks
            // until the last reference is removed.
            (allocated_bytes, 0, 0.25f32, true)
        } else {
            (0, allocated_bytes, 1.0f32, false)
        };

        Self {
            logical_bytes,
            allocated_bytes,
            shared_bytes,
            exclusive_bytes,
            potentially_reclaimable_bytes: exclusive_bytes,
            is_sparse,
            is_clone,
            clone_id,
            clone_refcnt,
            extent_uncertainty,
            reclaim_confidence,
        }
    }

    pub fn zero() -> Self {
        Self {
            logical_bytes: 0,
            allocated_bytes: 0,
            shared_bytes: 0,
            exclusive_bytes: 0,
            potentially_reclaimable_bytes: 0,
            is_sparse: false,
            is_clone: false,
            clone_id: None,
            clone_refcnt: 1,
            extent_uncertainty: false,
            reclaim_confidence: 1.0,
        }
    }

    pub fn accumulate(&mut self, other: &AllocationInfo) {
        self.logical_bytes = self.logical_bytes.saturating_add(other.logical_bytes);
        self.allocated_bytes = self.allocated_bytes.saturating_add(other.allocated_bytes);
        self.shared_bytes = self.shared_bytes.saturating_add(other.shared_bytes);
        self.exclusive_bytes = self.exclusive_bytes.saturating_add(other.exclusive_bytes);
        self.potentially_reclaimable_bytes = self
            .potentially_reclaimable_bytes
            .saturating_add(other.potentially_reclaimable_bytes);
        self.is_sparse = self.is_sparse || other.is_sparse;
        self.is_clone = self.is_clone || other.is_clone;
        self.extent_uncertainty = self.extent_uncertainty || other.extent_uncertainty;
    }
}
