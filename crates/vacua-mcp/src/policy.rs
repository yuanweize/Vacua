use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use vacua_api::{VacuaErrorCode, VacuaErrorResponse};

pub const MAX_PROPOSAL_CANDIDATES: usize = 200;
pub const MAX_SIMULATION_CANDIDATES: usize = 500;
pub const MAX_STRING_PARAM_LEN: usize = 512;
pub const MAX_CURSOR_LEN: usize = 512;

/// Path disclosure policy for machine API responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PathDisclosureMode {
    HomeRelative,
    Full,
    Redacted,
}

impl PathDisclosureMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "home-relative" | "homerelative" | "relative" => Some(Self::HomeRelative),
            "full" | "absolute" => Some(Self::Full),
            "redacted" | "mask" => Some(Self::Redacted),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HomeRelative => "home-relative",
            Self::Full => "full",
            Self::Redacted => "redacted",
        }
    }
}

/// Authoritative allowed root representation canonicalized and validated at startup.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AllowedRoot {
    pub root_id: String,
    pub canonical_path: PathBuf,
    pub display_name: String,
}

impl AllowedRoot {
    /// Create a validated, canonicalized allowed root.
    /// Fails closed if the path does not exist, cannot be canonicalized, or is not a directory.
    pub fn try_new(raw_path: &Path, home_dir: Option<&Path>) -> Result<Self, String> {
        let canonical_path = raw_path.canonicalize().map_err(|e| {
            format!(
                "Failed to canonicalize root '{}': {}",
                raw_path.display(),
                e
            )
        })?;

        if !canonical_path.is_dir() {
            return Err(format!(
                "Allowed root '{}' is not a directory",
                canonical_path.display()
            ));
        }

        let is_home = home_dir.is_some_and(|h| {
            if let Ok(can_h) = h.canonicalize() {
                can_h == canonical_path
            } else {
                h == canonical_path
            }
        });

        let (root_id, display_name) = if is_home {
            ("root-home".to_string(), "~".to_string())
        } else {
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"VACUA_MCP_ROOT_ID_V1:");
            hasher.update(canonical_path.as_os_str().as_encoded_bytes());
            let hash = hasher.finalize();
            let short_hash = &hash.to_hex()[..8];
            let id = format!("root-{}", short_hash);
            let display = format!("<root:{}>", id);
            (id, display)
        };

        Ok(Self {
            root_id,
            canonical_path,
            display_name,
        })
    }
}

/// Opaque structured pagination cursor binding query identity and scope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CursorV2 {
    pub v: u8,
    pub entity_kind: String,
    pub root_id: String,
    pub query_fingerprint: String,
    pub offset: usize,
    pub state_generation: u64,
}

/// Validate string input bounds.
pub fn validate_string_bound(s: &str, field_name: &str) -> Result<(), VacuaErrorResponse> {
    if s.len() > MAX_STRING_PARAM_LEN {
        return Err(VacuaErrorResponse::new(
            VacuaErrorCode::VacuaLimitExceeded,
            format!(
                "Field '{}' length ({} bytes) exceeds maximum limit of {} bytes",
                field_name,
                s.len(),
                MAX_STRING_PARAM_LEN
            ),
        ));
    }
    Ok(())
}

/// Encode binary payload to Base64URL without padding.
pub fn base64url_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((data.len() * 4).div_ceil(3));
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(ALPHABET[(b0 >> 2) as usize] as char);
        out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(b2 & 0x3f) as usize] as char);
        }
    }
    out
}

/// Decode Base64URL without padding into binary payload.
pub fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity((s.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;

    for c in s.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '-' => 62,
            '_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Server policy configuration and capability isolation boundary.
#[derive(Debug, Clone)]
pub struct McpPolicy {
    pub allowed_roots: Vec<AllowedRoot>,
    pub path_disclosure: PathDisclosureMode,
    pub max_results: usize,
    pub max_expensive_operations: usize,
    pub timeout: Duration,
    pub home_dir: Option<PathBuf>,
    pub allow_plan_export: bool,
    pub allow_system_app_metadata: bool,
    expensive_semaphore: Arc<Semaphore>,
}

impl McpPolicy {
    pub fn new(
        allowed_roots: Vec<AllowedRoot>,
        path_disclosure: PathDisclosureMode,
        max_results: usize,
        max_expensive_operations: usize,
        timeout: Duration,
        allow_plan_export: bool,
        allow_system_app_metadata: bool,
    ) -> Self {
        let home_dir = std::env::var_os("HOME").map(PathBuf::from);
        let max_results = if max_results == 0 {
            50
        } else {
            max_results.min(200)
        };
        let max_expensive = max_expensive_operations.clamp(1, 16);
        Self {
            allowed_roots,
            path_disclosure,
            max_results,
            max_expensive_operations: max_expensive,
            timeout,
            home_dir,
            allow_plan_export,
            allow_system_app_metadata,
            expensive_semaphore: Arc::new(Semaphore::new(max_expensive)),
        }
    }

    /// Check if target path falls within configured allowed roots.
    /// FAIL CLOSED:
    /// - If allowed_roots is empty -> false (never allow all!)
    /// - If path cannot be canonicalized (non-existent, broken symlink) -> false
    /// - If symlink escapes outside the allowed root -> false
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        if self.allowed_roots.is_empty() {
            return false;
        }

        let canonical = match path.canonicalize() {
            Ok(c) => c,
            Err(_) => return false,
        };

        for root in &self.allowed_roots {
            if canonical.starts_with(&root.canonical_path) {
                return true;
            }
        }
        false
    }

    /// Resolve an authoritative allowed root by optional root_id.
    /// If root_id is None, defaults to the primary (first) root.
    pub fn get_root(&self, root_id: Option<&str>) -> Result<&AllowedRoot, VacuaErrorResponse> {
        if self.allowed_roots.is_empty() {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaPolicyDenied,
                "No allowed filesystem roots configured on server",
            ));
        }

        if let Some(id) = root_id {
            self.allowed_roots
                .iter()
                .find(|r| r.root_id == id)
                .ok_or_else(|| {
                    VacuaErrorResponse::new(
                        VacuaErrorCode::VacuaNotFound,
                        format!("Configured root '{}' not found", id),
                    )
                })
        } else {
            Ok(&self.allowed_roots[0])
        }
    }

    /// Primary allowed root for default operations.
    pub fn primary_root(&self) -> Result<&AllowedRoot, VacuaErrorResponse> {
        self.get_root(None)
    }

    /// Acquire permit for expensive operations (e.g. content hashing / duplicate scan).
    pub async fn acquire_expensive_permit(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, VacuaErrorResponse> {
        self.expensive_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| {
                VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaBusy,
                    "Server busy: expensive operation capacity reached",
                )
            })
    }

    /// Generate a domain-separated opaque deterministic candidate ID.
    pub fn generate_candidate_id(&self, root_id: &str, relative_path: &Path) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"VACUA_MCP_CANDIDATE_ID_V1:");
        hasher.update(root_id.as_bytes());
        hasher.update(b":");
        hasher.update(relative_path.as_os_str().as_encoded_bytes());
        let hash = hasher.finalize();
        format!("cand-{}", &hash.to_hex()[..16])
    }

    /// Generate a domain-separated opaque deterministic duplicate group ID.
    pub fn generate_duplicate_group_id(&self, root_id: &str, content_hash: &str) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"VACUA_MCP_DUP_GROUP_ID_V1:");
        hasher.update(root_id.as_bytes());
        hasher.update(b":");
        hasher.update(content_hash.as_bytes());
        let hash = hasher.finalize();
        format!("dup-{}", &hash.to_hex()[..16])
    }

    /// Generate a domain-separated opaque deterministic duplicate member ID.
    pub fn generate_duplicate_member_id(&self, path: &Path) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"VACUA_MCP_MEMBER_ID_V1:");
        hasher.update(path.as_os_str().as_encoded_bytes());
        let hash = hasher.finalize();
        format!("mem-{}", &hash.to_hex()[..16])
    }

    /// Generate a domain-separated opaque deterministic application artifact ID.
    pub fn generate_artifact_id(&self, path: &Path) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"VACUA_MCP_ARTIFACT_ID_V1:");
        hasher.update(path.as_os_str().as_encoded_bytes());
        let hash = hasher.finalize();
        format!("art-{}", &hash.to_hex()[..16])
    }

    /// Format a filesystem path according to current disclosure mode.
    pub fn format_path(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();
        match self.path_disclosure {
            PathDisclosureMode::Full => self.sanitize_string(&path_str),
            PathDisclosureMode::HomeRelative => {
                if let Some(ref home) = self.home_dir {
                    if let Ok(rel) = path.strip_prefix(home) {
                        return self.sanitize_string(&format!("~/{}", rel.display()));
                    }
                }
                // If path is under a non-home configured allowed root, prefix with <root:{root_id}>
                for root in &self.allowed_roots {
                    if root.root_id != "root-home" {
                        if let Ok(rel) = path.strip_prefix(&root.canonical_path) {
                            return self.sanitize_string(&format!(
                                "<root:{}>/{}",
                                root.root_id,
                                rel.display()
                            ));
                        }
                    }
                }
                self.sanitize_string(&path_str)
            }
            PathDisclosureMode::Redacted => {
                if let Some(file_name) = path.file_name() {
                    let sanitized_name = self.sanitize_string(&file_name.to_string_lossy());
                    format!("<redacted-path>/{}", sanitized_name)
                } else {
                    "<redacted-path>".to_string()
                }
            }
        }
    }

    /// Sanitize text data from untrusted filesystem metadata:
    /// Strips control characters (C0, C1), ANSI escape codes, bidi overrides/isolates,
    /// while preserving normal UTF-8 characters (e.g. Chinese, Czech, emojis).
    pub fn sanitize_string(&self, s: &str) -> String {
        let mut clean = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            // Strip ANSI escape sequences \x1b[...]
            if c == '\x1b' {
                if let Some(&'[') = chars.peek() {
                    chars.next();
                    for next_c in chars.by_ref() {
                        if next_c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                    continue;
                }
            }

            // Strip bidi override / embedding / isolate controls
            match c {
                '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => continue,
                _ => {}
            }

            // Reject C0/C1 control characters except space
            if (c.is_control() && c != ' ') || ('\u{007F}'..='\u{009F}').contains(&c) {
                clean.push(' ');
            } else {
                clean.push(c);
            }
        }

        clean
    }

    /// Clamp requested pagination limit between 1 and max_results.
    pub fn clamp_limit(&self, limit: Option<usize>) -> usize {
        limit.unwrap_or(self.max_results).clamp(1, self.max_results)
    }

    /// Encode an opaque CursorV2 bound to query scope.
    pub fn encode_cursor_v2(
        entity_kind: &str,
        root_id: &str,
        query_fingerprint: &str,
        offset: usize,
        state_generation: u64,
    ) -> String {
        let cursor = CursorV2 {
            v: 2,
            entity_kind: entity_kind.to_string(),
            root_id: root_id.to_string(),
            query_fingerprint: query_fingerprint.to_string(),
            offset,
            state_generation,
        };
        let json = serde_json::to_vec(&cursor).unwrap_or_default();
        base64url_encode(&json)
    }

    /// Decode and validate an opaque CursorV2.
    /// Strictly rejects invalid formats, length > 512, or mismatched query scope.
    pub fn decode_cursor_v2(
        cursor_str: &str,
        expected_entity_kind: &str,
        expected_root_id: &str,
        expected_query_fingerprint: &str,
    ) -> Result<usize, VacuaErrorResponse> {
        if cursor_str.len() > MAX_CURSOR_LEN {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaLimitExceeded,
                format!(
                    "Cursor length exceeds maximum allowed limit of {} bytes",
                    MAX_CURSOR_LEN
                ),
            ));
        }

        let bytes = base64url_decode(cursor_str).ok_or_else(|| {
            VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInvalidArgument,
                "Malformed cursor: invalid base64url encoding",
            )
        })?;

        let cursor: CursorV2 = serde_json::from_slice(&bytes).map_err(|_| {
            VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInvalidArgument,
                "Malformed cursor: invalid JSON payload",
            )
        })?;

        if cursor.v != 2 {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInvalidArgument,
                format!("Unsupported cursor version: {}", cursor.v),
            ));
        }

        if cursor.entity_kind != expected_entity_kind {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInvalidArgument,
                format!(
                    "Cursor entity kind mismatch: expected '{}', found '{}'",
                    expected_entity_kind, cursor.entity_kind
                ),
            ));
        }

        if cursor.root_id != expected_root_id {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInvalidArgument,
                format!(
                    "Cursor root ID mismatch: expected '{}', found '{}'",
                    expected_root_id, cursor.root_id
                ),
            ));
        }

        if cursor.query_fingerprint != expected_query_fingerprint {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInvalidArgument,
                "Cursor query fingerprint mismatch: filters or sort criteria have changed",
            ));
        }

        Ok(cursor.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_empty_roots_fail_closed() {
        let policy = McpPolicy::new(
            vec![],
            PathDisclosureMode::Full,
            50,
            2,
            Duration::from_secs(10),
            false,
            false,
        );
        assert!(!policy.is_path_allowed(Path::new("/tmp")));
        assert!(!policy.is_path_allowed(Path::new("/etc")));
        assert!(policy.primary_root().is_err());
    }

    #[test]
    fn test_symlink_escape_denied() {
        let dir = tempdir().unwrap();
        let canonical_dir = dir.path().canonicalize().unwrap();
        let root = AllowedRoot::try_new(&canonical_dir, None).unwrap();

        let policy = McpPolicy::new(
            vec![root],
            PathDisclosureMode::Full,
            50,
            2,
            Duration::from_secs(10),
            false,
            false,
        );

        // Valid file inside root
        let inside_file = canonical_dir.join("inside.txt");
        std::fs::write(&inside_file, "hello").unwrap();
        assert!(policy.is_path_allowed(&inside_file));

        // Symlink pointing to /etc (outside allowed root)
        let link_to_etc = canonical_dir.join("link_to_etc");
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink("/etc", &link_to_etc);
            if link_to_etc.exists() {
                assert!(!policy.is_path_allowed(&link_to_etc));
            }
        }

        // Non-existent path fails closed
        assert!(!policy.is_path_allowed(&canonical_dir.join("non_existent.txt")));
    }

    #[test]
    fn test_cursor_v2_binding_and_validation() {
        let encoded = McpPolicy::encode_cursor_v2("candidate", "root-home", "fp-123", 42, 1);
        let decoded =
            McpPolicy::decode_cursor_v2(&encoded, "candidate", "root-home", "fp-123").unwrap();
        assert_eq!(decoded, 42);

        // Mismatched entity kind
        let err = McpPolicy::decode_cursor_v2(&encoded, "snapshot", "root-home", "fp-123");
        assert!(err.is_err());
        assert_eq!(err.unwrap_err().code, VacuaErrorCode::VacuaInvalidArgument);

        // Mismatched root ID
        let err = McpPolicy::decode_cursor_v2(&encoded, "candidate", "root-other", "fp-123");
        assert!(err.is_err());

        // Mismatched query fingerprint
        let err = McpPolicy::decode_cursor_v2(&encoded, "candidate", "root-home", "fp-456");
        assert!(err.is_err());

        // Corrupted base64
        let err =
            McpPolicy::decode_cursor_v2("!!!not-base64!!!", "candidate", "root-home", "fp-123");
        assert!(err.is_err());
    }

    #[test]
    fn test_sanitization_preserves_unicode_and_strips_controls() {
        let policy = McpPolicy::new(
            vec![],
            PathDisclosureMode::Full,
            50,
            2,
            Duration::from_secs(10),
            false,
            false,
        );
        let raw = "\x1b[31mRed\x1b[0m \u{202E}reversed\u{202C} 测\t试 文件 🚀";
        let cleaned = policy.sanitize_string(raw);
        assert!(!cleaned.contains("\x1b[31m"));
        assert!(!cleaned.contains('\u{202E}'));
        assert!(cleaned.contains("测 试 文件 🚀"));
    }
}
