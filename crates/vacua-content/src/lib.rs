//! Vacua Content Identity & Duplicate Intelligence Engine.
//!
//! A scientifically correct, staged, APFS-aware duplicate discovery
//! and content fingerprint caching engine for macOS.

pub mod cache;
pub mod cloud;
pub mod engine;
pub mod group;
pub mod identity;
pub mod staged;
pub mod stats;

pub use cache::FingerprintCache;
pub use cloud::{
    is_cloud_placeholder, is_cloud_placeholder_fd, is_dataless_flags, SF_DATALESS, UF_COMPRESSED,
};
pub use engine::DuplicateEngine;
pub use group::{DuplicateGroup, DuplicateMember, DuplicatePlanEstimate};
pub use identity::{
    compute_full_fingerprint_toctou, compute_sample_fingerprint, ContentError, ContentIdentity,
    FingerprintState, PhysicalRelation, DIRECT_FULL_HASH_THRESHOLD, DOMAIN_SEPARATION_SAMPLE_V1,
    FINGERPRINT_VERSION_FULL, FINGERPRINT_VERSION_SAMPLE, SAMPLE_WINDOW_SIZE,
};
pub use staged::{
    run_staged_duplicate_pipeline, verify_duplicate_pair_before_deletion, DuplicateScanOptions,
};
pub use stats::DedupStats;
