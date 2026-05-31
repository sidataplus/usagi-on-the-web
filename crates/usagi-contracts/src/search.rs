use serde::{Deserialize, Serialize};

use crate::catalog::{ConceptSummary, Provenance};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchConceptsRequest {
    pub q: String,
    pub mode: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub filters: serde_json::Value,
    #[serde(default)]
    pub hybrid: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub rank: usize,
    pub concept: ConceptSummary,
    pub scores: serde_json::Value,
    pub component_ranks: serde_json::Value,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchConceptsResponse {
    pub query: String,
    pub mode: String,
    pub results: Vec<SearchResult>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchBatchRequest {
    pub mode: String,
    #[serde(default = "default_limit")]
    pub limit_per_item: usize,
    pub items: Vec<SearchBatchItemRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchBatchItemRequest {
    pub id: String,
    pub q: String,
    #[serde(default)]
    pub filters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchBatchResponse {
    pub mode: String,
    pub items: Vec<SearchBatchItemResponse>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchBatchItemResponse {
    pub id: String,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchExplainRequest {
    pub q: String,
    pub concept_id: i64,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchExplainResponse {
    pub query: String,
    pub concept_id: i64,
    pub explanation: serde_json::Value,
}

fn default_limit() -> usize {
    20
}
