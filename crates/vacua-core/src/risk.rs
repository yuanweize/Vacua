use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    /// Safe to automatically propose for cleanup. Reproducible generated artifact.
    Safe = 0,
    /// Reconstructable, but incurs rebuild friction, network download, or offline workflow interruption.
    Review = 1,
    /// High-entropy, stale settings, or ambiguous leftover requiring careful confirmation.
    Caution = 2,
    /// Inviolable system file, user document, security key, or active database. Never delete.
    Protected = 3,
    /// Unclassified or indeterminate evidence. Never auto-clean.
    Unknown = 4,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Safe => write!(f, "SAFE"),
            RiskLevel::Review => write!(f, "REVIEW"),
            RiskLevel::Caution => write!(f, "CAUTION"),
            RiskLevel::Protected => write!(f, "PROTECTED"),
            RiskLevel::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl RiskLevel {
    pub fn is_automatable(&self) -> bool {
        matches!(self, RiskLevel::Safe)
    }

    pub fn is_protected_or_unknown(&self) -> bool {
        matches!(self, RiskLevel::Protected | RiskLevel::Unknown)
    }

    pub fn is_reversible(&self) -> bool {
        !self.is_protected_or_unknown()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecommendationValue {
    Negligible = 0,
    Low = 1,
    Medium = 2,
    High = 3,
}

impl std::fmt::Display for RecommendationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecommendationValue::Negligible => write!(f, "NEGLIGIBLE"),
            RecommendationValue::Low => write!(f, "LOW"),
            RecommendationValue::Medium => write!(f, "MEDIUM"),
            RecommendationValue::High => write!(f, "HIGH"),
        }
    }
}
