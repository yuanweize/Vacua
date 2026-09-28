use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedEntry {
    pub path: PathBuf,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub inode: u64,
    pub device_id: u64,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_sparse: bool,
    pub nlink: u64,
    pub mtime_sec: i64,
    pub ctime_sec: i64,
}

impl ScannedEntry {
    #[cfg(unix)]
    pub fn from_path(path: PathBuf) -> std::io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(&path)?;
        let is_symlink = meta.file_type().is_symlink();
        let is_dir = meta.is_dir();

        let logical_bytes = meta.len();
        // On macOS / APFS / HFS+, blocks are 512-byte units
        let allocated_bytes = meta.blocks() * 512;
        let is_sparse = !is_dir && !is_symlink && (allocated_bytes < logical_bytes);

        Ok(Self {
            path,
            logical_bytes,
            allocated_bytes,
            inode: meta.ino(),
            device_id: meta.dev(),
            is_dir,
            is_symlink,
            is_sparse,
            nlink: meta.nlink(),
            mtime_sec: meta.mtime(),
            ctime_sec: meta.ctime(),
        })
    }
}
