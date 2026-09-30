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
        }
    }
}

impl Default for RebuildEvidence {
    fn default() -> Self {
        Self::new()
    }
}
