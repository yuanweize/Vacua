use serde::{Deserialize, Serialize};

/// Detailed allocation metrics distinguishing logical content size from
/// physical blocks allocated on disk (APFS / HFS+ blocks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationInfo {
    /// Nominal byte size of the file contents (st_size).
    pub logical_bytes: u64,

    /// True physical bytes allocated on storage (st_blocks * 512).
    pub allocated_bytes: u64,

    /// Estimated bytes that can be freed if reclaimed.
    pub potentially_reclaimable_bytes: u64,

    /// Whether this file exhibits sparse allocation (allocated < logical).
    pub is_sparse: bool,

    /// True if physical saving is uncertain due to potential APFS clonefile extent sharing.
    pub extent_uncertainty: bool,
}

impl AllocationInfo {
    pub fn new(logical_bytes: u64, allocated_bytes: u64, extent_uncertainty: bool) -> Self {
        let is_sparse = allocated_bytes < logical_bytes;
        // Conservative reclaimable estimation: cannot reclaim more than physically allocated
        let potentially_reclaimable_bytes = allocated_bytes;

        Self {
            logical_bytes,
            allocated_bytes,
            potentially_reclaimable_bytes,
            is_sparse,
            extent_uncertainty,
        }
    }

    pub fn zero() -> Self {
        Self {
            logical_bytes: 0,
            allocated_bytes: 0,
            potentially_reclaimable_bytes: 0,
            is_sparse: false,
            extent_uncertainty: false,
        }
    }

    pub fn accumulate(&mut self, other: &AllocationInfo) {
        self.logical_bytes = self.logical_bytes.saturating_add(other.logical_bytes);
        self.allocated_bytes = self.allocated_bytes.saturating_add(other.allocated_bytes);
        self.potentially_reclaimable_bytes = self
            .potentially_reclaimable_bytes
            .saturating_add(other.potentially_reclaimable_bytes);
        self.is_sparse = self.is_sparse || other.is_sparse;
        self.extent_uncertainty = self.extent_uncertainty || other.extent_uncertainty;
    }
}
