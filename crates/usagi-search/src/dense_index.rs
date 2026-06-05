use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use usagi_catalog::store::CatalogStore;
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_common::manifest::{ArtifactManifest, ManifestFile};
use usagi_contracts::catalog::ConceptSummary;
use usagi_contracts::search::SearchResult;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DenseDocument {
    pub concept_id: i64,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SapbertDenseBuildOptions {
    pub artifact_dir: PathBuf,
    pub catalog_artifact_id: String,
    pub model_artifact_id: String,
    pub documents: Vec<DenseDocument>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SapbertDenseBuildSummary {
    pub sapbert_artifact_id: String,
    pub document_count: usize,
    pub dimension: usize,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DenseSearchOptions {
    pub artifact_dir: PathBuf,
    pub catalog_db_path: PathBuf,
    pub query_vector: Vec<f32>,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct SapbertPrecomputedBuildOptions {
    pub artifact_dir: PathBuf,
    pub catalog_artifact_id: String,
    pub model_artifact_id: String,
    pub embeddings_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrecomputedDenseEmbeddings {
    pub documents: Vec<DenseDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrecomputedDenseQueries {
    pub queries: Vec<PrecomputedDenseQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrecomputedDenseQuery {
    pub q: String,
    pub vector: Vec<f32>,
}

pub fn build_sapbert_dense_index(
    options: SapbertDenseBuildOptions,
) -> Result<SapbertDenseBuildSummary> {
    let dimension = validate_documents(&options.documents)?;
    if options.artifact_dir.exists() {
        fs::remove_dir_all(&options.artifact_dir)?;
    }
    fs::create_dir_all(&options.artifact_dir)?;

    let index = new_cosine_index(dimension)?;
    index
        .reserve(options.documents.len())
        .map_err(dense_error)?;

    let mut concept_ids = Vec::with_capacity(options.documents.len());
    for document in &options.documents {
        index
            .add(document.concept_id as u64, &document.vector)
            .map_err(dense_error)?;
        concept_ids.push(document.concept_id);
    }

    let usearch_path = options.artifact_dir.join("sapbert_cls.usearch");
    index
        .save(path_to_str(&usearch_path)?)
        .map_err(dense_error)?;
    fs::write(
        options.artifact_dir.join("concept_ids.arrow"),
        serde_json::to_vec(&concept_ids)?,
    )?;

    let manifest_path = options.artifact_dir.join("manifest.json");
    let manifest = ArtifactManifest {
        artifact_id: "local-sapbert-cls-v1".to_string(),
        artifact_kind: "usearch-sapbert-cls".to_string(),
        schema_version: "usagi-sapbert-cls-v1".to_string(),
        api_version: "0.1.0".to_string(),
        created_at: Utc::now().to_rfc3339(),
        inputs: Vec::new(),
        outputs: vec![
            ManifestFile {
                path: "sapbert_cls.usearch".to_string(),
                sha256: None,
                content_type: None,
            },
            ManifestFile {
                path: "concept_ids.arrow".to_string(),
                sha256: None,
                content_type: None,
            },
        ],
        extra: Some(serde_json::json!({
            "catalog": {
                "artifact_id": options.catalog_artifact_id
            },
            "model": {
                "model_artifact_id": options.model_artifact_id,
                "pooling": "cls",
                "dimension": dimension,
                "normalized": true
            },
            "index": {
                "engine": "usearch",
                "metric": "cosine",
                "document_count": options.documents.len()
            }
        })),
    };
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    Ok(SapbertDenseBuildSummary {
        sapbert_artifact_id: "local-sapbert-cls-v1".to_string(),
        document_count: options.documents.len(),
        dimension,
        manifest_path,
    })
}

pub fn build_sapbert_dense_index_from_precomputed(
    options: SapbertPrecomputedBuildOptions,
) -> Result<SapbertDenseBuildSummary> {
    let embeddings: PrecomputedDenseEmbeddings =
        serde_json::from_slice(&fs::read(&options.embeddings_path)?).map_err(|err| {
            UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                format!(
                    "precomputed SapBERT embedding artifact {} is not valid JSON: {err}",
                    options.embeddings_path.display()
                ),
            )
        })?;
    build_sapbert_dense_index(SapbertDenseBuildOptions {
        artifact_dir: options.artifact_dir,
        catalog_artifact_id: options.catalog_artifact_id,
        model_artifact_id: options.model_artifact_id,
        documents: embeddings.documents,
    })
}

pub fn search_sapbert_dense(options: DenseSearchOptions) -> Result<Vec<SearchResult>> {
    let dimension = read_dimension(&options.artifact_dir.join("manifest.json"))?;
    if options.query_vector.len() != dimension {
        return Err(UsagiError::bad_request(format!(
            "query vector dimension {} does not match index dimension {dimension}",
            options.query_vector.len()
        )));
    }
    let index = new_cosine_index(dimension)?;
    index
        .load(path_to_str(
            &options.artifact_dir.join("sapbert_cls.usearch"),
        )?)
        .map_err(dense_error)?;

    let matches = index
        .search(&options.query_vector, options.limit)
        .map_err(dense_error)?;
    let catalog = CatalogStore::open(options.catalog_db_path)?;
    let mut results = Vec::new();
    for (idx, (key, distance)) in matches
        .keys
        .iter()
        .zip(matches.distances.iter())
        .enumerate()
    {
        let Some(concept) = catalog.get_concept(*key as i64)? else {
            continue;
        };
        results.push(SearchResult {
            rank: idx + 1,
            concept: ConceptSummary::from(concept),
            scores: serde_json::json!({
                "tantivy": null,
                "sapbert": 1.0_f32 - *distance,
                "rrf": null
            }),
            component_ranks: serde_json::json!({
                "sapbert": idx + 1
            }),
            method: "sapbert_cls".to_string(),
        });
    }
    Ok(results)
}

pub fn lookup_precomputed_sapbert_query_vector(
    query_embeddings_path: impl AsRef<std::path::Path>,
    q: &str,
) -> Result<Option<Vec<f32>>> {
    let queries: PrecomputedDenseQueries =
        serde_json::from_slice(&fs::read(query_embeddings_path.as_ref())?).map_err(|err| {
            UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                format!(
                    "precomputed SapBERT query artifact {} is not valid JSON: {err}",
                    query_embeddings_path.as_ref().display()
                ),
            )
        })?;
    Ok(queries
        .queries
        .into_iter()
        .find(|query| query.q == q)
        .map(|query| query.vector))
}

pub fn search_sapbert_dense_from_precomputed_query(
    mut options: DenseSearchOptions,
    query_embeddings_path: impl AsRef<std::path::Path>,
    q: &str,
) -> Result<Vec<SearchResult>> {
    let query_vector = lookup_precomputed_sapbert_query_vector(query_embeddings_path, q)?
        .ok_or_else(|| {
            UsagiError::new(
                ErrorCode::EmbeddingFailed,
                format!("missing precomputed SapBERT query embedding for {q}"),
            )
        })?;
    options.query_vector = query_vector;
    search_sapbert_dense(options)
}

fn validate_documents(documents: &[DenseDocument]) -> Result<usize> {
    let first = documents
        .first()
        .ok_or_else(|| UsagiError::bad_request("at least one dense document is required"))?;
    let dimension = first.vector.len();
    if dimension == 0 {
        return Err(UsagiError::bad_request(
            "dense vector dimension must be greater than zero",
        ));
    }
    if documents
        .iter()
        .any(|document| document.vector.len() != dimension)
    {
        return Err(UsagiError::bad_request(
            "all dense vectors must have the same dimension",
        ));
    }
    Ok(dimension)
}

fn new_cosine_index(dimension: usize) -> Result<Index> {
    Index::new(&IndexOptions {
        dimensions: dimension,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        connectivity: 0,
        expansion_add: 0,
        expansion_search: 0,
        multi: false,
    })
    .map_err(dense_error)
}

fn read_dimension(manifest_path: &PathBuf) -> Result<usize> {
    let manifest: ArtifactManifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
    manifest
        .extra
        .and_then(|extra| {
            extra
                .get("model")
                .and_then(|model| model.get("dimension"))
                .and_then(|dimension| dimension.as_u64())
        })
        .map(|dimension| dimension as usize)
        .ok_or_else(|| {
            UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                "SapBERT manifest is missing model.dimension",
            )
        })
}

fn path_to_str(path: &std::path::Path) -> Result<&str> {
    path.to_str().ok_or_else(|| {
        UsagiError::bad_request(format!("path is not valid UTF-8: {}", path.display()))
    })
}

fn dense_error(err: impl std::fmt::Display) -> UsagiError {
    UsagiError::internal(err.to_string())
}
