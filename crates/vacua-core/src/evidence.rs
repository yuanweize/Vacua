use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceSource {
    PathSemantic,
    BundleIdentifier,
    ApplicationPresence,
    PackageReceipt,
    ProcessState,
    FileAge,
    FilesystemMetadata,
    Reconstructability,
    RuleEngine,
    UserHistory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub source: EvidenceSource,
    pub signal: String,
    pub weight: f32,
    pub explanation: String,
}

impl Evidence {
    pub fn new(
        source: EvidenceSource,
        signal: impl Into<String>,
        weight: f32,
        explanation: impl Into<String>,
    ) -> Self {
        Self {
            source,
            signal: signal.into(),
            weight,
            explanation: explanation.into(),
        }
    }
}
