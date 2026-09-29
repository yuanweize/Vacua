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

    /// Exact private bytes uniquely attributable to this file queried directly from
    /// Darwin kernel via ATTR_CMNEXT_PRIVATESIZE on APFS, if available.
    pub kernel_private_bytes: Option<u64>,
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
            kernel_private_bytes: None,
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
            kernel_private_bytes: None,
        }
    }

    pub fn with_kernel_private_bytes(mut self, private_bytes: Option<u64>) -> Self {
        self.kernel_private_bytes = private_bytes;
        if let Some(priv_bytes) = private_bytes {
            self.exclusive_bytes = priv_bytes;
            self.potentially_reclaimable_bytes = priv_bytes;
            self.shared_bytes = self.allocated_bytes.saturating_sub(priv_bytes);
            if self.is_clone {
                self.reclaim_confidence = if priv_bytes > 0 { 0.9 } else { 0.1 };
            }
        }
        self
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
            kernel_private_bytes: None,
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
        if let (Some(a), Some(b)) = (self.kernel_private_bytes, other.kernel_private_bytes) {
            self.kernel_private_bytes = Some(a.saturating_add(b));
        } else if other.kernel_private_bytes.is_some() {
            self.kernel_private_bytes = other.kernel_private_bytes;
        }
    }

    /// Confirmed physical bytes guaranteed to be freed immediately upon deletion (exclusive blocks).
    pub fn confirmed_freeable_bytes(&self) -> u64 {
        self.exclusive_bytes
    }

    /// Estimated physical bytes accounting for probabilistic reclaim confidence.
    pub fn estimated_freeable_bytes(&self) -> u64 {
        (self.allocated_bytes as f32 * self.reclaim_confidence) as u64
    }

    /// Theoretical upper bound of space freed if all shared clone family references are cleaned.
    pub fn upper_bound_freeable_bytes(&self) -> u64 {
        self.allocated_bytes
    }
}
