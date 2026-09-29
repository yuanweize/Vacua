use crate::cloud::is_cloud_placeholder_fd;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use thiserror::Error;

pub const SAMPLE_WINDOW_SIZE: usize = 64 * 1024; // 64 KiB
pub const DIRECT_FULL_HASH_THRESHOLD: u64 = 192 * 1024; // 192 KiB
pub const DOMAIN_SEPARATION_SAMPLE_V1: &[u8] = b"VACUA_SAMPLE_V1";
pub const FINGERPRINT_VERSION_SAMPLE: &str = "vacua-sample-v1";
pub const FINGERPRINT_VERSION_FULL: &str = "blake3-full-v1";

#[derive(Error, Debug)]
pub enum ContentError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File content changed during read (TOCTOU violation)")]
    ChangedDuringRead,

    #[error("Skipped dataless cloud placeholder")]
    SkippedCloudPlaceholder,

    #[error("Skipped special file or symlink")]
    SkippedSpecialFile,

    #[error("Database error: {0}")]
    Database(#[from] vacua_index::IndexError),

    #[error("Duplicate group error: {0}")]
    Group(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FingerprintState {
    MetadataOnly,
    Sampled,
    FullHash,
    ChangedDuringRead,
    SkippedCloudPlaceholder,
    SkippedSpecialFile,
    IOError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalRelation {
    Independent,
    HardlinkSameInode,
    APFSCloneFamily,
    UnknownShared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentIdentity {
    pub logical_size: u64,
    pub sample_fingerprint: Option<String>,
    pub full_fingerprint: Option<String>,
    pub device_id: u64,
    pub inode: u64,
    pub mtime_sec: i64,
    pub mtime_nsec: i64,
    pub ctime_sec: i64,
    pub ctime_nsec: i64,
    pub clone_id: Option<u64>,
    pub clone_refcnt: u32,
    pub fingerprint_state: FingerprintState,
}

/// Computes a deterministic partial / sample fingerprint for fast candidate filtering.
/// If the file size is below DIRECT_FULL_HASH_THRESHOLD, performs a full hash directly
/// to avoid multiple disk seek penalties.
pub fn compute_sample_fingerprint(
    path: &Path,
    file_size: u64,
) -> Result<(String, FingerprintState, u64), ContentError> {
    if file_size <= DIRECT_FULL_HASH_THRESHOLD {
        let mut file = File::open(path)?;
        let fd = file.as_raw_fd();
        if is_cloud_placeholder_fd(fd) {
            return Ok((String::new(), FingerprintState::SkippedCloudPlaceholder, 0));
        }

        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut total_read = 0u64;

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            total_read += n as u64;
        }

        let digest = hasher.finalize().to_hex().to_string();
        return Ok((digest, FingerprintState::FullHash, total_read));
    }

    let mut file = File::open(path)?;
    let fd = file.as_raw_fd();
    if is_cloud_placeholder_fd(fd) {
        return Ok((String::new(), FingerprintState::SkippedCloudPlaceholder, 0));
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(DOMAIN_SEPARATION_SAMPLE_V1);
    hasher.update(&file_size.to_le_bytes());

    let mut buffer = vec![0u8; SAMPLE_WINDOW_SIZE];
    let mut total_read = 0u64;

    // Window 1: Start (offset 0)
    let offset1 = 0u64;
    file.seek(SeekFrom::Start(offset1))?;
    let n1 = file.read(&mut buffer)?;
    hasher.update(&offset1.to_le_bytes());
    hasher.update(&(n1 as u64).to_le_bytes());
    hasher.update(&buffer[..n1]);
    total_read += n1 as u64;

    // Window 2: Middle
    let offset2 = (file_size / 2).saturating_sub(SAMPLE_WINDOW_SIZE as u64 / 2);
    file.seek(SeekFrom::Start(offset2))?;
    let n2 = file.read(&mut buffer)?;
    hasher.update(&offset2.to_le_bytes());
    hasher.update(&(n2 as u64).to_le_bytes());
    hasher.update(&buffer[..n2]);
    total_read += n2 as u64;

    // Window 3: End
    let offset3 = file_size.saturating_sub(SAMPLE_WINDOW_SIZE as u64);
    file.seek(SeekFrom::Start(offset3))?;
    let n3 = file.read(&mut buffer)?;
    hasher.update(&offset3.to_le_bytes());
    hasher.update(&(n3 as u64).to_le_bytes());
    hasher.update(&buffer[..n3]);
    total_read += n3 as u64;

    let digest = hasher.finalize().to_hex().to_string();
    Ok((digest, FingerprintState::Sampled, total_read))
}

/// Computes the full cryptographic BLAKE3 content digest with strict TOCTOU verification
/// on the opened file descriptor (comparing dev, ino, size, and nanosecond mtime/ctime
/// before and after hashing).
#[allow(clippy::too_many_arguments)]
pub fn compute_full_fingerprint_toctou(
    path: &Path,
    expected_dev: u64,
    expected_ino: u64,
    expected_size: u64,
    expected_mtime_sec: i64,
    expected_mtime_nsec: i64,
    expected_ctime_sec: i64,
    expected_ctime_nsec: i64,
) -> Result<(String, FingerprintState, u64), ContentError> {
    let mut file = File::open(path)?;
    let fd = file.as_raw_fd();

    if is_cloud_placeholder_fd(fd) {
        return Ok((String::new(), FingerprintState::SkippedCloudPlaceholder, 0));
    }

    // 1. Initial fstat verification on open descriptor
    let initial_meta = file.metadata()?;
    if initial_meta.dev() != expected_dev
        || initial_meta.ino() != expected_ino
        || initial_meta.len() != expected_size
        || initial_meta.mtime() != expected_mtime_sec
        || initial_meta.mtime_nsec() != expected_mtime_nsec
        || initial_meta.ctime() != expected_ctime_sec
        || initial_meta.ctime_nsec() != expected_ctime_nsec
    {
        return Ok((String::new(), FingerprintState::ChangedDuringRead, 0));
    }

    // 2. Sequential buffered stream reading
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 128 * 1024]; // 128 KiB streaming buffer
    let mut total_bytes_read = 0u64;

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        total_bytes_read += n as u64;
    }

    // 3. Post-hash fstat verification on the exact same descriptor
    let post_meta = file.metadata()?;
    if post_meta.dev() != expected_dev
        || post_meta.ino() != expected_ino
        || post_meta.len() != expected_size
        || post_meta.mtime() != expected_mtime_sec
        || post_meta.mtime_nsec() != expected_mtime_nsec
        || post_meta.ctime() != expected_ctime_sec
        || post_meta.ctime_nsec() != expected_ctime_nsec
    {
        return Ok((
            String::new(),
            FingerprintState::ChangedDuringRead,
            total_bytes_read,
        ));
    }

    let digest = hasher.finalize().to_hex().to_string();
    Ok((digest, FingerprintState::FullHash, total_bytes_read))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_sample_hash_small_file_matches_full_hash() {
        let mut file = NamedTempFile::new().unwrap();
        let payload = b"Hello, Vacua Content Identity!";
        file.write_all(payload).unwrap();
        file.flush().unwrap();

        let (sample_digest, state, bytes_read) =
            compute_sample_fingerprint(file.path(), payload.len() as u64).unwrap();
        assert_eq!(state, FingerprintState::FullHash);
        assert_eq!(bytes_read, payload.len() as u64);

        let mut hasher = blake3::Hasher::new();
        hasher.update(payload);
        assert_eq!(sample_digest, hasher.finalize().to_hex().to_string());
    }

    #[test]
    fn test_sample_hash_large_file_domain_separated() {
        let mut file = NamedTempFile::new().unwrap();
        let size = 500 * 1024; // 500 KiB > 192 KiB
        let mut data = vec![0u8; size];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }
        file.write_all(&data).unwrap();
        file.flush().unwrap();

        let (sample_digest, state, bytes_read) =
            compute_sample_fingerprint(file.path(), size as u64).unwrap();
        assert_eq!(state, FingerprintState::Sampled);
        assert_eq!(bytes_read, (3 * SAMPLE_WINDOW_SIZE) as u64);
        assert!(!sample_digest.is_empty());
    }

    #[test]
    fn test_toctou_verification_detects_mutation() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"Original content").unwrap();
        file.flush().unwrap();

        let meta = file.path().metadata().unwrap();

        // Pass invalid expected mtime
        let (_, state, _) = compute_full_fingerprint_toctou(
            file.path(),
            meta.dev(),
            meta.ino(),
            meta.len(),
            meta.mtime() + 999,
            meta.mtime_nsec(),
            meta.ctime(),
            meta.ctime_nsec(),
        )
        .unwrap();

        assert_eq!(state, FingerprintState::ChangedDuringRead);
    }
}
