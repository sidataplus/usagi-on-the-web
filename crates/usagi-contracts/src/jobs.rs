use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    CatalogBuild,
    TantivyBuild,
    SapbertBuild,
    ThirawatDocEmbed,
    TachiomBuild,
    MapperDrugsBatch,
}

impl JobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CatalogBuild => "catalog_build",
            Self::TantivyBuild => "tantivy_build",
            Self::SapbertBuild => "sapbert_build",
            Self::ThirawatDocEmbed => "thirawat_doc_embed",
            Self::TachiomBuild => "tachiom_build",
            Self::MapperDrugsBatch => "mapper_drugs_batch",
        }
    }
}

impl std::fmt::Display for JobKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for JobKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "catalog_build" => Ok(Self::CatalogBuild),
            "tantivy_build" => Ok(Self::TantivyBuild),
            "sapbert_build" => Ok(Self::SapbertBuild),
            "thirawat_doc_embed" => Ok(Self::ThirawatDocEmbed),
            "tachiom_build" => Ok(Self::TachiomBuild),
            "mapper_drugs_batch" => Ok(Self::MapperDrugsBatch),
            other => Err(format!("unknown job kind {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    SucceededWithErrors,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::SucceededWithErrors => "succeeded_with_errors",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for JobState {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "succeeded_with_errors" => Ok(Self::SucceededWithErrors),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown job state {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobItemState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
}

impl JobItemState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobDto {
    pub id: String,
    pub kind: JobKind,
    pub queue: String,
    pub state: JobState,
    pub stage: Option<String>,
    pub processed: i64,
    pub total: i64,
    pub failed: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobCreateResponse {
    pub job_id: String,
    pub state: JobState,
    pub status_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobEventDto {
    pub id: String,
    pub job_id: String,
    pub seq: i64,
    pub level: String,
    pub message: String,
    pub payload: Option<serde_json::Value>,
    pub created_at: String,
}
