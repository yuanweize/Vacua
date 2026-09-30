use crate::ecosystem::DeveloperEcosystem;
use crate::kind::DeveloperArtifactKind;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

const PROJECT_ID_DOMAIN_SEPARATOR: &[u8] = b"VACUA_DEV_PROJ_V1";
const ARTIFACT_ID_DOMAIN_SEPARATOR: &[u8] = b"VACUA_DEV_ART_V1";

/// Opaque, deterministic project identity derived from BLAKE3 hash of
/// root_id, raw root-relative path bytes, and primary ecosystem.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct DeveloperProjectId(pub String);

impl DeveloperProjectId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn from_raw_relative(
        root_id: &str,
        raw_relative_path: &[u8],
        ecosystem: DeveloperEcosystem,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(PROJECT_ID_DOMAIN_SEPARATOR);
        hasher.update(b":");
        hasher.update(root_id.as_bytes());
        hasher.update(b":");
        hasher.update(raw_relative_path);
        hasher.update(b":");
        hasher.update(ecosystem.as_str().as_bytes());
        let hash = hasher.finalize();
        let hex_prefix = &hash.to_hex()[..32];
        Self(format!("devproj_{}", hex_prefix))
    }

    pub fn from_relative_path(
        root_id: &str,
        relative_path: &Path,
        ecosystem: DeveloperEcosystem,
    ) -> Self {
        let raw_bytes = path_to_raw_bytes(relative_path);
        Self::from_raw_relative(root_id, &raw_bytes, ecosystem)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeveloperProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Opaque, deterministic artifact identity derived from BLAKE3 hash of
/// project_id, raw root-relative path bytes, and artifact kind.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct DeveloperArtifactId(pub String);

impl DeveloperArtifactId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn from_raw_relative(
        project_id: &DeveloperProjectId,
        raw_relative_path: &[u8],
        kind: DeveloperArtifactKind,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ARTIFACT_ID_DOMAIN_SEPARATOR);
        hasher.update(b":");
        hasher.update(project_id.as_str().as_bytes());
        hasher.update(b":");
        hasher.update(raw_relative_path);
        hasher.update(b":");
        hasher.update(kind.as_str().as_bytes());
        let hash = hasher.finalize();
        let hex_prefix = &hash.to_hex()[..32];
        Self(format!("devart_{}", hex_prefix))
    }

    pub fn from_relative_path(
        project_id: &DeveloperProjectId,
        relative_path: &Path,
        kind: DeveloperArtifactKind,
    ) -> Self {
        let raw_bytes = path_to_raw_bytes(relative_path);
        Self::from_raw_relative(project_id, &raw_bytes, kind)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeveloperArtifactId {
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
