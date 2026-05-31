use serde::{Deserialize, Serialize};

use crate::catalog::{ConceptSummary, Provenance};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugQueryRequest {
    pub source_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_code: Option<String>,
    #[serde(default = "default_mapper_mode")]
    pub mode: String,
    #[serde(default = "default_candidate_top_k")]
    pub candidate_top_k: usize,
    #[serde(default = "default_rerank_top_n")]
    pub rerank_top_n: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub post_rank: serde_json::Value,
    #[serde(default)]
    pub normalization: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperCandidate {
    pub rank: usize,
    pub concept: ConceptSummary,
    pub scores: serde_json::Value,
    pub features: serde_json::Value,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugQueryResponse {
    pub query: serde_json::Value,
    pub mode: String,
    pub candidates: Vec<MapperCandidate>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugBatchRequest {
    #[serde(default = "default_mapper_mode")]
    pub mode: String,
    #[serde(default = "default_candidate_top_k")]
    pub candidate_top_k: usize,
    #[serde(default = "default_rerank_top_n")]
    pub rerank_top_n: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub items: Vec<MapperDrugBatchItemRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugBatchJobRequest {
    pub idempotency_key: String,
    #[serde(default = "default_mapper_mode")]
    pub mode: String,
    #[serde(default = "default_candidate_top_k")]
    pub candidate_top_k: usize,
    #[serde(default = "default_rerank_top_n")]
    pub rerank_top_n: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub items: Vec<MapperDrugBatchItemRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugBatchItemRequest {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_code: Option<String>,
    pub source_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_frequency: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugBatchResponse {
    pub mode: String,
    pub items: Vec<MapperDrugBatchItemResponse>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugBatchItemResponse {
    pub id: String,
    pub candidates: Vec<MapperCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugExplainRequest {
    pub source_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_code: Option<String>,
    pub concept_id: i64,
    #[serde(default = "default_mapper_mode")]
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MapperDrugExplainResponse {
    pub query: serde_json::Value,
    pub concept: ConceptSummary,
    pub scores: serde_json::Value,
    pub features: serde_json::Value,
    pub token_debug: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThirawatBuildEmbeddingsJobRequest {
    pub idempotency_key: String,
    #[serde(default = "default_thirawat_model_artifact_id")]
    pub model_artifact_id: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub scope: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TachiomBuildJobRequest {
    pub idempotency_key: String,
    #[serde(default = "default_thirawat_doc_embedding_artifact_id")]
    pub doc_embedding_artifact_id: String,
    #[serde(default)]
    pub overwrite: bool,
}

fn default_mapper_mode() -> String {
    "thirawat_tachiom".to_string()
}

fn default_candidate_top_k() -> usize {
    200
}

fn default_rerank_top_n() -> usize {
    100
}

fn default_limit() -> usize {
    20
}

fn default_thirawat_model_artifact_id() -> String {
    "sidataplus-thirawat-sapbert-merged-v1".to_string()
}

fn default_thirawat_doc_embedding_artifact_id() -> String {
    "athena-local-thirawat-drug-docemb-v1".to_string()
}
