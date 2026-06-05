use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Concept {
    pub concept_id: i64,
    pub concept_name: String,
    pub domain_id: String,
    pub vocabulary_id: String,
    pub concept_class_id: String,
    pub standard_concept: String,
    pub concept_code: String,
    pub valid_start_date: Option<String>,
    pub valid_end_date: Option<String>,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConceptSummary {
    pub concept_id: i64,
    pub concept_name: String,
    pub domain_id: String,
    pub vocabulary_id: String,
    pub concept_class_id: String,
    pub standard_concept: String,
    pub concept_code: String,
}

impl From<Concept> for ConceptSummary {
    fn from(value: Concept) -> Self {
        Self {
            concept_id: value.concept_id,
            concept_name: value.concept_name,
            domain_id: value.domain_id,
            vocabulary_id: value.vocabulary_id,
            concept_class_id: value.concept_class_id,
            standard_concept: value.standard_concept,
            concept_code: value.concept_code,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Provenance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_artifact_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConceptResponse {
    pub concept: Concept,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchConceptsRequest {
    pub concept_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchConceptsResponse {
    pub concepts: Vec<ConceptSummary>,
    pub missing: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelatedConcept {
    pub concept: ConceptSummary,
    pub min_levels_of_separation: Option<i64>,
    pub max_levels_of_separation: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConceptRelationship {
    pub relationship_id: String,
    pub direction: String,
    pub target_concept: ConceptSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogStatus {
    pub status: String,
    pub api_version: String,
    pub catalog: Option<CatalogStatusDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogStatusDetail {
    pub artifact_id: String,
    pub vocabulary_version: String,
    pub concept_count: i64,
    pub scope: CatalogScope,
    pub manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogScope {
    pub standard_concept: String,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogBuildJobRequest {
    pub athena_dir: String,
    pub idempotency_key: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub vocabulary_version: Option<String>,
    #[serde(default)]
    pub artifact_id: Option<String>,
}
