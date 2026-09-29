use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Canonical error codes returned by the Vacua machine API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VacuaErrorCode {
    VacuaNotFound,
    VacuaStaleState,
    VacuaPolicyDenied,
    VacuaProtected,
    VacuaLimitExceeded,
    VacuaBusy,
    VacuaInternal,
    VacuaInvalidArgument,
}

impl VacuaErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VacuaNotFound => "VACUA_NOT_FOUND",
            Self::VacuaStaleState => "VACUA_STALE_STATE",
            Self::VacuaPolicyDenied => "VACUA_POLICY_DENIED",
            Self::VacuaProtected => "VACUA_PROTECTED",
            Self::VacuaLimitExceeded => "VACUA_LIMIT_EXCEEDED",
            Self::VacuaBusy => "VACUA_BUSY",
            Self::VacuaInternal => "VACUA_INTERNAL",
            Self::VacuaInvalidArgument => "VACUA_INVALID_ARGUMENT",
        }
    }
}

impl std::fmt::Display for VacuaErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Structured error envelope for MCP and JSON responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VacuaErrorResponse {
    pub code: VacuaErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl VacuaErrorResponse {
    pub fn new(code: VacuaErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}
