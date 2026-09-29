use crate::identity::{ContentError, FINGERPRINT_VERSION_FULL, FINGERPRINT_VERSION_SAMPLE};
use std::path::Path;
use vacua_index::{
    CachedFingerprint, ContentFingerprintRecord, FingerprintCacheStats, IndexDatabase,
};

pub struct FingerprintCache<'a> {
    db: &'a IndexDatabase,
}

impl<'a> FingerprintCache<'a> {
    pub fn new(db: &'a IndexDatabase) -> Self {
        Self { db }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get(
        &self,
        dev: u64,
        ino: u64,
        canonical_path: &Path,
        logical_size: u64,
        mtime_sec: i64,
        mtime_nsec: i64,
        ctime_sec: i64,
        ctime_nsec: i64,
    ) -> Result<Option<CachedFingerprint>, ContentError> {
        let res = self.db.get_content_fingerprint(
            dev,
            ino,
            canonical_path,
            logical_size,
            mtime_sec,
            mtime_nsec,
            ctime_sec,
            ctime_nsec,
            Some(FINGERPRINT_VERSION_SAMPLE),
            Some(FINGERPRINT_VERSION_FULL),
        )?;
        Ok(res)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn put_sample(
        &self,
        dev: u64,
        ino: u64,
        canonical_path: &Path,
        logical_size: u64,
        mtime_sec: i64,
        mtime_nsec: i64,
        ctime_sec: i64,
        ctime_nsec: i64,
        sample_hash: &str,
    ) -> Result<(), ContentError> {
        let now = chrono::Utc::now().timestamp();
        let record = ContentFingerprintRecord {
            device_id: dev,
            inode: ino,
            canonical_path: canonical_path.to_path_buf(),
            logical_size,
            mtime_sec,
            mtime_nsec,
            ctime_sec,
            ctime_nsec,
            sample_hash: Some(sample_hash.to_string()),
            sample_version: Some(FINGERPRINT_VERSION_SAMPLE.to_string()),
            full_hash: None,
            full_version: None,
            hash_algorithm: "BLAKE3".to_string(),
            observed_at: now,
        };
        self.db.upsert_content_fingerprint(&record)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn put_full(
        &self,
        dev: u64,
        ino: u64,
        canonical_path: &Path,
        logical_size: u64,
        mtime_sec: i64,
        mtime_nsec: i64,
        ctime_sec: i64,
        ctime_nsec: i64,
        sample_hash: Option<&str>,
        full_hash: &str,
    ) -> Result<(), ContentError> {
        let now = chrono::Utc::now().timestamp();
        let record = ContentFingerprintRecord {
            device_id: dev,
            inode: ino,
            canonical_path: canonical_path.to_path_buf(),
            logical_size,
            mtime_sec,
            mtime_nsec,
            ctime_sec,
            ctime_nsec,
            sample_hash: sample_hash.map(|s| s.to_string()),
            sample_version: sample_hash.map(|_| FINGERPRINT_VERSION_SAMPLE.to_string()),
            full_hash: Some(full_hash.to_string()),
            full_version: Some(FINGERPRINT_VERSION_FULL.to_string()),
            hash_algorithm: "BLAKE3".to_string(),
            observed_at: now,
        };
        self.db.upsert_content_fingerprint(&record)?;
        Ok(())
    }

    pub fn stats(&self) -> Result<FingerprintCacheStats, ContentError> {
        Ok(self.db.get_fingerprint_stats()?)
    }

    pub fn prune(&self) -> Result<usize, ContentError> {
        Ok(self.db.prune_missing_fingerprints()?)
    }
}
