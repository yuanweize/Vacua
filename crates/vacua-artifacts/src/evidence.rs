use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RebuildConfidence {
    Unknown,
    Partial,
    Strong,
    Verified,
}

impl RebuildConfidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Partial => "partial",
            Self::Strong => "strong",
            Self::Verified => "verified",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown Confidence",
            Self::Partial => "Partial Evidence",
            Self::Strong => "Strong Evidence",
            Self::Verified => "Verified Evidence",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "unknown" => Some(Self::Unknown),
            "partial" => Some(Self::Partial),
            "strong" => Some(Self::Strong),
            "verified" => Some(Self::Verified),
            _ => None,
        }
    }
}

impl std::fmt::Display for RebuildConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActiveProjectState {
    Active,
    Dormant,
    Unknown,
}

impl ActiveProjectState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Dormant => "dormant",
            Self::Unknown => "unknown",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Active => "Active (<7 days activity)",
            Self::Dormant => "Dormant (>=7 days inactive)",
            Self::Unknown => "Unknown State",
        }
    }
}

impl std::fmt::Display for ActiveProjectState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Comprehensive, evidence-based causal rebuild model for developer artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RebuildEvidence {
    pub manifest_present: bool,
    pub manifest_path: Option<String>,
    pub lockfile_present: bool,
    pub lockfile_path: Option<String>,
    pub known_artifact_convention: bool,
    pub project_root_known: bool,
    pub toolchain_identified: Option<String>,
    pub active_project_state: ActiveProjectState,
    pub reconstruction_confidence: RebuildConfidence,
    pub rebuild_command_template: Option<String>,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub active_guard_deferred: bool,
}

impl RebuildEvidence {
    pub fn new() -> Self {
        Self {
            manifest_present: false,
            manifest_path: None,
            lockfile_present: false,
            lockfile_path: None,
            known_artifact_convention: false,
            project_root_known: false,
            toolchain_identified: None,
            active_project_state: ActiveProjectState::Unknown,
            reconstruction_confidence: RebuildConfidence::Unknown,
            rebuild_command_template: None,
            reasons: Vec::new(),
            active_guard_deferred: false,
        }
    }
}

impl Default for RebuildEvidence {
    fn default() -> Self {
        Self::new()
    }
}

/// Active process and recent activity guard to protect in-flight development builds.
#[derive(Debug, Clone)]
pub struct ActiveProcessGuard {
    active_process_indicators: Vec<String>,
}

impl ActiveProcessGuard {
    pub fn new() -> Self {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
        let mut indicators = Vec::new();

        // 1. Simulation / test override hook
        if let Ok(sim) = std::env::var("VACUA_SIMULATED_ACTIVE_PROCESS_PATH") {
            if !sim.is_empty() {
                indicators.push(sim);
            }
        }

        // 2. Query live OS processes for build compilers and package managers
        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .with_cwd(UpdateKind::OnlyIfNotSet),
        );

        let build_binary_names = [
            "rustc",
            "cargo",
            "cargo-build",
            "cargo-check",
            "cargo-clippy",
            "xcodebuild",
            "swiftc",
            "swift-frontend",
            "node",
            "npm",
            "pnpm",
            "yarn",
            "bun",
            "python",
            "python3",
            "pytest",
            "uv",
            "clang",
            "clang++",
            "gcc",
            "g++",
            "cmake",
            "ninja",
            "gradle",
            "gradlew",
            "mvn",
        ];

        for proc in sys.processes().values() {
            let proc_name = proc.name().to_string_lossy().to_lowercase();
            let is_build_tool = build_binary_names
                .iter()
                .any(|&b| proc_name == b || proc_name.starts_with(b));
            if is_build_tool {
                if let Some(cwd) = proc.cwd() {
                    indicators.push(cwd.to_string_lossy().to_string());
                }
                for arg in proc.cmd() {
                    let s = arg.to_string_lossy();
                    if s.starts_with('/') || s.contains('/') {
                        indicators.push(s.to_string());
                    }
                }
            }
        }

        Self {
            active_process_indicators: indicators,
        }
    }

    /// Evaluates whether an artifact or project path is currently active.
    /// Returns true if a live build process references the path, or if recent mtime indicates activity.
    pub fn is_active_target(
        &self,
        project_path: &std::path::Path,
        artifact_path: &std::path::Path,
        most_recent_mtime: u64,
    ) -> bool {
        let proj_canon = project_path
            .canonicalize()
            .unwrap_or_else(|_| project_path.to_path_buf());
        let art_canon = artifact_path
            .canonicalize()
            .unwrap_or_else(|_| artifact_path.to_path_buf());

        // Check if path is referenced by any running compiler or build process
        for ind in &self.active_process_indicators {
            if ind.is_empty() {
                continue;
            }
            let ind_path = std::path::Path::new(ind);
            let ind_canon = ind_path
                .canonicalize()
                .unwrap_or_else(|_| ind_path.to_path_buf());
            if ind_canon.parent().is_none() {
                continue;
            }

            // A build process is active if its working directory or file target is at or inside the project/artifact
            if ind_canon.starts_with(&proj_canon)
                || ind_canon.starts_with(&art_canon)
                || ind_canon == proj_canon
                || ind_canon == art_canon
            {
                return true;
            }
        }

        // Check if there was filesystem activity within the last 180 seconds (3 minutes)
        if most_recent_mtime > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_secs();
            if now.saturating_sub(most_recent_mtime) < 180 {
                return true;
            }
        }

        false
    }
}

impl Default for ActiveProcessGuard {
    fn default() -> Self {
        Self::new()
    }
}
