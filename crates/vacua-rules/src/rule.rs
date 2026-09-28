use serde::{Deserialize, Serialize};
use vacua_core::candidate::CandidateCategory;
use vacua_core::risk::RiskLevel;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardType {
    ProcessNotRunning,
    MinAgeDays,
    PathExists,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleGuard {
    #[serde(rename = "type")]
    pub guard_type: GuardType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMatch {
    pub path: String,
    pub file_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: CandidateCategory,
    pub risk: RiskLevel,
    pub reconstructable: bool,
    pub rebuild_consequence: Option<String>,
    #[serde(default)]
    pub matches: Vec<RuleMatch>,
    #[serde(default)]
    pub guards: Vec<RuleGuard>,
}
