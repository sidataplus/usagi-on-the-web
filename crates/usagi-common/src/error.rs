use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::request::generate_request_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    BadRequest,
    Unauthorized,
    NotFound,
    CatalogNotReady,
    IndexNotReady,
    ModelNotReady,
    IncompatibleArtifact,
    JobNotFound,
    JobCancelled,
    JobFailed,
    EmbeddingFailed,
    TachiomFailed,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BadRequest => "BAD_REQUEST",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::NotFound => "NOT_FOUND",
            Self::CatalogNotReady => "CATALOG_NOT_READY",
            Self::IndexNotReady => "INDEX_NOT_READY",
            Self::ModelNotReady => "MODEL_NOT_READY",
            Self::IncompatibleArtifact => "INCOMPATIBLE_ARTIFACT",
            Self::JobNotFound => "JOB_NOT_FOUND",
            Self::JobCancelled => "JOB_CANCELLED",
            Self::JobFailed => "JOB_FAILED",
            Self::EmbeddingFailed => "EMBEDDING_FAILED",
            Self::TachiomFailed => "TACHIOM_FAILED",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsagiError {
    code: ErrorCode,
    message: String,
    details: Option<serde_json::Value>,
    request_id: Option<String>,
}

impl UsagiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
            request_id: None,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }

    pub fn incompatible_artifact(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::IncompatibleArtifact, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn details(&self) -> Option<&serde_json::Value> {
        self.details.as_ref()
    }

    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

impl fmt::Display for UsagiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for UsagiError {}

impl From<std::io::Error> for UsagiError {
    fn from(value: std::io::Error) -> Self {
        Self::internal(value.to_string())
    }
}

impl From<serde_json::Error> for UsagiError {
    fn from(value: serde_json::Error) -> Self {
        Self::internal(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub request_id: String,
}

impl From<UsagiError> for ErrorEnvelope {
    fn from(value: UsagiError) -> Self {
        Self {
            error: ErrorBody {
                code: value.code,
                message: value.message,
                details: value.details,
                request_id: value.request_id.unwrap_or_else(generate_request_id),
            },
        }
    }
}

pub type Result<T> = std::result::Result<T, UsagiError>;

pub fn details_object(
    items: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
) -> serde_json::Value {
    let mut map = BTreeMap::new();
    for (key, value) in items {
        map.insert(key.into(), serde_json::Value::String(value.into()));
    }
    serde_json::to_value(map).unwrap_or(serde_json::Value::Null)
}
