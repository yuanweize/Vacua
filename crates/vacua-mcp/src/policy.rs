use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Path disclosure policy for machine API responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Server policy configuration and capability isolation boundary.
#[derive(Debug, Clone)]
pub struct McpPolicy {
    pub allowed_roots: Vec<PathBuf>,
    pub path_disclosure: PathDisclosureMode,
    pub max_results: usize,
    pub max_expensive_operations: usize,
    pub timeout: Duration,
    pub home_dir: Option<PathBuf>,
    expensive_semaphore: Arc<Semaphore>,
}

impl McpPolicy {
    pub fn new(
        allowed_roots: Vec<PathBuf>,
        path_disclosure: PathDisclosureMode,
        max_results: usize,
        max_expensive_operations: usize,
        timeout: Duration,
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
            expensive_semaphore: Arc::new(Semaphore::new(max_expensive)),
        }
    }

    /// Check if target path falls within configured allowed roots.
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        if self.allowed_roots.is_empty() {
            return true;
        }

        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        for root in &self.allowed_roots {
            let canonical_root = root.canonicalize().unwrap_or_else(|_| root.clone());
            if canonical.starts_with(&canonical_root) {
                return true;
            }
        }
        false
    }

    /// Primary allowed root for default operations.
    pub fn primary_root(&self) -> PathBuf {
        if let Some(first) = self.allowed_roots.first() {
            first.clone()
        } else if let Some(ref home) = self.home_dir {
            home.clone()
        } else {
            PathBuf::from(".")
        }
    }

    /// Acquire permit for expensive operations (e.g. content hashing / duplicate scan).
    pub async fn acquire_expensive_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.expensive_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore closed")
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
    /// Strips control characters (NUL, backspace, carriage return), ANSI escape codes,
    /// ensuring purely well-formed display strings that cannot corrupt terminals or protocol.
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

            // Reject control characters except space
            if c.is_control() && c != ' ' {
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

    /// Encode an opaque cursor representing item offset.
    pub fn encode_cursor(offset: usize) -> String {
        let raw = format!("cursor:vacua:v1:{}", offset);
        // Base64-like hex encoding
        let mut hex = String::new();
        for b in raw.as_bytes() {
            hex.push_str(&format!("{:02x}", b));
        }
        hex
    }

    /// Decode an opaque cursor into item offset. Returns None if invalid.
    #[allow(clippy::manual_is_multiple_of)]
    pub fn decode_cursor(cursor: &str) -> Option<usize> {
        if cursor.len() % 2 != 0 {
            return None;
        }

        let mut bytes = Vec::new();
        for i in (0..cursor.len()).step_by(2) {
            let byte_str = &cursor[i..i + 2];
            let byte = u8::from_str_radix(byte_str, 16).ok()?;
            bytes.push(byte);
        }

        let decoded = String::from_utf8(bytes).ok()?;
        let prefix = "cursor:vacua:v1:";
        if !decoded.starts_with(prefix) {
            return None;
        }

        decoded[prefix.len()..].parse::<usize>().ok()
    }
}
