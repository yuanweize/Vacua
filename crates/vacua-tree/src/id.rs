use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

const NODE_ID_DOMAIN_SEPARATOR: &[u8] = b"VACUA_STORAGE_NODE_V1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StorageNodeId(pub String);

impl StorageNodeId {
    /// Constructs a StorageNodeId wrapping an existing opaque ID string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Creates a new opaque node ID from root ID and raw relative path bytes.
    pub fn from_raw_relative(root_id: &str, raw_relative_path: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(NODE_ID_DOMAIN_SEPARATOR);
        hasher.update(b":");
        hasher.update(root_id.as_bytes());
        hasher.update(b":");
        hasher.update(raw_relative_path);
        let hash = hasher.finalize();
        // Use 32 hex chars (16 bytes) prefixed with "stn_"
        let hex_prefix = &hash.to_hex()[..32];
        StorageNodeId(format!("stn_{}", hex_prefix))
    }

    /// Computes node ID from a relative Path using raw OsStr bytes on Unix.
    pub fn from_relative_path(root_id: &str, relative_path: &Path) -> Self {
        let raw_bytes = path_to_raw_bytes(relative_path);
        Self::from_raw_relative(root_id, &raw_bytes)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StorageNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Extracts raw bytes from a Path without lossy UTF-8 conversion on Unix.
pub fn path_to_raw_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        path.to_string_lossy().as_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_node_id_stability_and_domain_separation() {
        let root = "root_abc";
        let rel_a = PathBuf::from("Library/Caches/com.apple.Safari");
        let id1 = StorageNodeId::from_relative_path(root, &rel_a);
        let id2 = StorageNodeId::from_relative_path(root, &rel_a);
        assert_eq!(id1, id2);
        assert!(id1.as_str().starts_with("stn_"));

        // Different root yields different node ID for identical relative path
        let id_diff_root = StorageNodeId::from_relative_path("root_xyz", &rel_a);
        assert_ne!(id1, id_diff_root);
    }

    #[test]
    fn test_node_id_does_not_leak_path() {
        let sensitive = "Users/alice/Secret Name/Finance";
        let id = StorageNodeId::from_raw_relative("root_1", sensitive.as_bytes());
        let id_str = id.as_str();

        assert!(!id_str.contains("alice"));
        assert!(!id_str.contains("Secret"));
        assert!(!id_str.contains("Finance"));
        assert!(!id_str.contains("Users"));
    }

    #[test]
    fn test_non_utf8_path_identity() {
        #[cfg(unix)]
        {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;

            let invalid_utf8_bytes = b"foo\xff\xfe\xfdbar";
            let os_str = OsStr::from_bytes(invalid_utf8_bytes);
            let path = Path::new(os_str);

            let id = StorageNodeId::from_relative_path("root_1", path);
            assert!(id.as_str().starts_with("stn_"));
            assert_eq!(id.as_str().len(), 36); // "stn_" (4) + 32 hex chars
        }
    }
}
