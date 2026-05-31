use std::path::Path;

use serde::{Deserialize, Serialize};
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_contracts::catalog::{ConceptSummary, Provenance};
use usagi_contracts::mapper::{
    MapperCandidate, MapperDrugBatchItemResponse, MapperDrugBatchRequest, MapperDrugBatchResponse,
    MapperDrugExplainResponse, MapperDrugQueryResponse,
};

use crate::artifact::{validate_tachiom_artifact, TachiomArtifactPaths};
use crate::bimaxsim::exact_bimaxsim;
use crate::tiebreak::{rank_near_ties, TieBreakCandidate, TieBreakOptions};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapperRuntimeOptions {
    pub candidate_top_k: usize,
    pub rerank_top_n: usize,
    pub limit: usize,
    pub epsilon: f32,
    pub tiebreak_top_n: usize,
}

impl Default for MapperRuntimeOptions {
    fn default() -> Self {
        Self {
            candidate_top_k: 200,
            rerank_top_n: 100,
            limit: 20,
            epsilon: 0.01,
            tiebreak_top_n: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TachiomFixtureIndex {
    pub documents: Vec<TachiomFixtureDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TachiomFixtureDocument {
    pub concept: ConceptSummary,
    pub token_vectors: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrecomputedQueryEmbeddings {
    pub items: Vec<PrecomputedQueryEmbedding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrecomputedQueryEmbedding {
    pub source_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_code: Option<String>,
    pub token_vectors: Vec<Vec<f32>>,
}

#[derive(Debug, Clone)]
struct RetrievedDocument {
    concept: ConceptSummary,
    token_vectors: Vec<Vec<f32>>,
    tachiom_maxsim: f32,
    bimaxsim: f32,
}

pub fn map_drug_query_from_precomputed(
    source_name: &str,
    source_code: Option<&str>,
    query_embeddings_path: impl AsRef<Path>,
    tachiom_index_dir: impl AsRef<Path>,
    options: MapperRuntimeOptions,
) -> Result<MapperDrugQueryResponse> {
    let embedding =
        load_precomputed_query_embedding(query_embeddings_path.as_ref(), source_name, source_code)?;
    map_drug_query_with_vectors(
        source_name,
        source_code,
        embedding.token_vectors,
        tachiom_index_dir,
        options,
    )
}

pub fn map_drug_explain_from_precomputed(
    source_name: &str,
    source_code: Option<&str>,
    concept_id: i64,
    query_embeddings_path: impl AsRef<Path>,
    tachiom_index_dir: impl AsRef<Path>,
    options: MapperRuntimeOptions,
) -> Result<MapperDrugExplainResponse> {
    let embedding =
        load_precomputed_query_embedding(query_embeddings_path.as_ref(), source_name, source_code)?;
    map_drug_explain_with_vectors(
        source_name,
        source_code,
        concept_id,
        embedding.token_vectors,
        tachiom_index_dir,
        options,
    )
}

pub fn map_drug_batch_from_precomputed(
    request: MapperDrugBatchRequest,
    query_embeddings_path: impl AsRef<Path>,
    tachiom_index_dir: impl AsRef<Path>,
) -> Result<MapperDrugBatchResponse> {
    let query_embeddings_path = query_embeddings_path.as_ref();
    let tachiom_index_dir = tachiom_index_dir.as_ref();
    let mut items = Vec::with_capacity(request.items.len());
    for item in request.items {
        let item_id = item.id;
        match map_drug_query_from_precomputed(
            &item.source_name,
            item.source_code.as_deref(),
            query_embeddings_path,
            tachiom_index_dir,
            MapperRuntimeOptions {
                candidate_top_k: request.candidate_top_k,
                rerank_top_n: request.rerank_top_n,
                limit: request.limit,
                ..MapperRuntimeOptions::default()
            },
        ) {
            Ok(response) => items.push(MapperDrugBatchItemResponse {
                id: item_id,
                candidates: response.candidates,
                error: None,
            }),
            Err(err) => items.push(MapperDrugBatchItemResponse {
                id: item_id,
                candidates: Vec::new(),
                error: Some(error_value(&err)),
            }),
        }
    }
    Ok(MapperDrugBatchResponse {
        mode: request.mode,
        items,
        provenance: Provenance {
            catalog_artifact_id: None,
            model_artifact_id: Some("sidataplus/THIRAWAT-SapBERT".to_string()),
            index_artifact_id: Some(
                tachiom_index_dir
                    .join("manifest.json")
                    .display()
                    .to_string(),
            ),
        },
    })
}

pub fn map_drug_explain_with_vectors(
    source_name: &str,
    source_code: Option<&str>,
    concept_id: i64,
    query_vectors: Vec<Vec<f32>>,
    tachiom_index_dir: impl AsRef<Path>,
    options: MapperRuntimeOptions,
) -> Result<MapperDrugExplainResponse> {
    let mut explain_options = options;
    explain_options.limit = explain_options.limit.max(explain_options.candidate_top_k);
    let response = map_drug_query_with_vectors(
        source_name,
        source_code,
        query_vectors,
        tachiom_index_dir,
        explain_options,
    )?;
    let candidate = response
        .candidates
        .into_iter()
        .find(|candidate| candidate.concept.concept_id == concept_id)
        .ok_or_else(|| UsagiError::not_found("concept was not found in mapper candidates"))?;
    Ok(MapperDrugExplainResponse {
        query: response.query,
        concept: candidate.concept,
        scores: candidate.scores,
        features: candidate.features,
        token_debug: serde_json::json!({
            "enabled": false,
            "message": "Token-level debug is disabled by default"
        }),
    })
}

pub fn map_drug_query_with_vectors(
    source_name: &str,
    source_code: Option<&str>,
    query_vectors: Vec<Vec<f32>>,
    tachiom_index_dir: impl AsRef<Path>,
    options: MapperRuntimeOptions,
) -> Result<MapperDrugQueryResponse> {
    let tachiom_index_dir = tachiom_index_dir.as_ref();
    validate_tachiom_artifact(TachiomArtifactPaths {
        index_dir: tachiom_index_dir.to_path_buf(),
    })?;
    let index = load_fixture_tachiom_index(tachiom_index_dir)?;
    let mut retrieved = retrieve_and_rerank(&query_vectors, index, options)?;
    let tie_candidates: Vec<TieBreakCandidate> = retrieved
        .iter()
        .map(|candidate| TieBreakCandidate {
            concept_id: candidate.concept.concept_id,
            concept_name: candidate.concept.concept_name.clone(),
            bimaxsim: candidate.bimaxsim,
            tachiom_maxsim: candidate.tachiom_maxsim,
        })
        .collect();
    let ranked = rank_near_ties(
        source_name,
        tie_candidates,
        TieBreakOptions {
            epsilon: options.epsilon,
            top_n: options.tiebreak_top_n,
        },
    );

    let mut candidates = Vec::new();
    for (index, ranked_candidate) in ranked.into_iter().take(options.limit).enumerate() {
        let retrieved_candidate = retrieved
            .iter_mut()
            .find(|item| item.concept.concept_id == ranked_candidate.concept_id)
            .ok_or_else(|| UsagiError::internal("ranked candidate missing retrieved document"))?;
        candidates.push(MapperCandidate {
            rank: index + 1,
            concept: retrieved_candidate.concept.clone(),
            scores: serde_json::json!({
                "tachiom_maxsim": retrieved_candidate.tachiom_maxsim,
                "bimaxsim": ranked_candidate.bimaxsim,
                "tie_breaker": ranked_candidate.tie_breaker,
                "final": ranked_candidate.final_score
            }),
            features: serde_json::to_value(ranked_candidate.features)?,
            method: "thirawat_tachiom_bimaxsim_tiebreak".to_string(),
        });
    }

    Ok(MapperDrugQueryResponse {
        query: query_json(source_name, source_code),
        mode: "thirawat_tachiom".to_string(),
        candidates,
        provenance: Provenance {
            catalog_artifact_id: None,
            model_artifact_id: Some("sidataplus/THIRAWAT-SapBERT".to_string()),
            index_artifact_id: Some(
                tachiom_index_dir
                    .join("manifest.json")
                    .display()
                    .to_string(),
            ),
        },
    })
}

fn error_value(err: &UsagiError) -> serde_json::Value {
    serde_json::json!({
        "code": err.code().as_str(),
        "message": err.message()
    })
}

pub fn load_precomputed_query_embedding(
    path: &Path,
    source_name: &str,
    source_code: Option<&str>,
) -> Result<PrecomputedQueryEmbedding> {
    let artifact: PrecomputedQueryEmbeddings = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|err| {
            UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                format!(
                    "precomputed query embedding artifact {} is not valid JSON: {err}",
                    path.display()
                ),
            )
        })?;
    artifact
        .items
        .into_iter()
        .find(|item| {
            item.source_name == source_name
                && source_code
                    .map(|code| item.source_code.as_deref() == Some(code))
                    .unwrap_or(true)
        })
        .ok_or_else(|| {
            UsagiError::new(
                ErrorCode::EmbeddingFailed,
                format!("missing precomputed query embedding for {source_name}"),
            )
        })
}

fn load_fixture_tachiom_index(index_dir: &Path) -> Result<TachiomFixtureIndex> {
    let index_path = index_dir.join("index.bin");
    serde_json::from_slice(&std::fs::read(&index_path)?).map_err(|err| {
        UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "Tachiom fixture index {} is not valid JSON: {err}",
                index_path.display()
            ),
        )
    })
}

fn retrieve_and_rerank(
    query_vectors: &[Vec<f32>],
    index: TachiomFixtureIndex,
    options: MapperRuntimeOptions,
) -> Result<Vec<RetrievedDocument>> {
    if options.candidate_top_k == 0 || options.rerank_top_n == 0 || options.limit == 0 {
        return Ok(Vec::new());
    }
    let mut retrieved = Vec::with_capacity(index.documents.len());
    for document in index.documents {
        let score = exact_bimaxsim(query_vectors, &document.token_vectors)?;
        retrieved.push(RetrievedDocument {
            concept: document.concept,
            token_vectors: document.token_vectors,
            tachiom_maxsim: score.query_to_document,
            bimaxsim: score.score,
        });
    }
    retrieved.sort_by(|left, right| {
        right
            .tachiom_maxsim
            .total_cmp(&left.tachiom_maxsim)
            .then_with(|| left.concept.concept_id.cmp(&right.concept.concept_id))
    });
    retrieved.truncate(options.candidate_top_k);

    for candidate in retrieved.iter_mut().take(options.rerank_top_n) {
        candidate.bimaxsim = exact_bimaxsim(query_vectors, &candidate.token_vectors)?.score;
    }
    retrieved.sort_by(|left, right| {
        right
            .bimaxsim
            .total_cmp(&left.bimaxsim)
            .then_with(|| right.tachiom_maxsim.total_cmp(&left.tachiom_maxsim))
            .then_with(|| left.concept.concept_id.cmp(&right.concept.concept_id))
    });
    retrieved.truncate(options.rerank_top_n);
    Ok(retrieved)
}

fn query_json(source_name: &str, source_code: Option<&str>) -> serde_json::Value {
    let query_text = match source_code {
        Some(code) => format!("{source_name} ({code})"),
        None => source_name.to_string(),
    };
    serde_json::json!({
        "source_name": source_name,
        "source_code": source_code,
        "query_text": query_text
    })
}
