use serde::{Deserialize, Serialize};

/// Comprehensive metrics and performance statistics for a duplicate scan session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DedupStats {
    pub files_seen: u64,
    pub eligible_files: u64,

    pub size_unique_files: u64,
    pub size_collision_files: u64,

    pub hardlinks_collapsed: u64,
    pub hardlink_aliases_collapsed: u64,
    pub hash_reads_avoided_by_hardlinks: u64,
    pub clone_family_members: u64,

    pub special_files_skipped: u64,
    pub io_errors_skipped: u64,

    pub sampled_files: u64,
    pub sample_bytes_read: u64,

    pub full_hashed_files: u64,
    pub full_hash_bytes_read: u64,

    pub sample_cache_hits: u64,
    pub sample_cache_misses: u64,
    pub full_cache_hits: u64,
    pub full_cache_misses: u64,
    pub cache_write_failures: u64,

    #[serde(default)]
    pub full_hash_cache_hits: u64,
    #[serde(default)]
    pub full_hash_cache_misses: u64,

    pub cloud_placeholders_skipped: u64,

    pub duplicate_groups: usize,
    pub duplicate_members: usize,

    pub logical_duplicate_bytes: u64,
    pub confirmed_reclaimable_bytes: u64,
    pub estimated_reclaimable_bytes: u64,
    pub upper_bound_reclaimable_bytes: u64,

    pub elapsed_ms: u64,
}
