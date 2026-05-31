use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_common::manifest::ArtifactManifest;
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
    let mut retrieved = retrieve_candidates(&query_vectors, tachiom_index_dir, options)?;
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

fn retrieve_candidates(
    query_vectors: &[Vec<f32>],
    index_dir: &Path,
    options: MapperRuntimeOptions,
) -> Result<Vec<RetrievedDocument>> {
    if options.candidate_top_k == 0 || options.rerank_top_n == 0 || options.limit == 0 {
        return Ok(Vec::new());
    }
    match tachiom_engine(index_dir)?.as_str() {
        "tachiom-cli" => retrieve_with_tachiom_cli(query_vectors, index_dir, options),
        "fixture-exact-maxsim" | "tachiom-fixture" | "" => retrieve_from_fixture_index(
            query_vectors,
            load_fixture_tachiom_index(index_dir)?,
            options,
        ),
        other => Err(UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!("unsupported Tachiom retrieval engine {other:?}"),
        )),
    }
}

fn retrieve_from_fixture_index(
    query_vectors: &[Vec<f32>],
    index: TachiomFixtureIndex,
    options: MapperRuntimeOptions,
) -> Result<Vec<RetrievedDocument>> {
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
    rerank_retrieved(query_vectors, retrieved, options)
}

fn rerank_retrieved(
    query_vectors: &[Vec<f32>],
    mut retrieved: Vec<RetrievedDocument>,
    options: MapperRuntimeOptions,
) -> Result<Vec<RetrievedDocument>> {
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

fn retrieve_with_tachiom_cli(
    query_vectors: &[Vec<f32>],
    index_dir: &Path,
    options: MapperRuntimeOptions,
) -> Result<Vec<RetrievedDocument>> {
    let search_bin = std::env::var("TACHIOM_SEARCH_BIN")
        .map(PathBuf::from)
        .map_err(|_| {
            UsagiError::new(
                ErrorCode::TachiomFailed,
                "TACHIOM_SEARCH_BIN is required for tachiom-cli indexes",
            )
        })?;
    if !search_bin.exists() {
        return Err(UsagiError::new(
            ErrorCode::TachiomFailed,
            format!("Tachiom search binary is missing {}", search_bin.display()),
        ));
    }
    let docs = load_doc_embedding_sidecar(index_dir)?;
    let work_dir = create_tachiom_query_work_dir()?;
    let query_path = work_dir.join("query.npy");
    let results_path = work_dir.join("results.tsv");
    write_query_npy(&query_path, query_vectors)?;
    let output = Command::new(&search_bin)
        .arg("-i")
        .arg(index_dir.join("index.bin"))
        .arg("-q")
        .arg(&query_path)
        .arg("-o")
        .arg(&results_path)
        .arg("--k")
        .arg(options.candidate_top_k.to_string())
        .arg("--num-runs")
        .arg("1")
        .output()?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&work_dir);
        return Err(UsagiError::new(
            ErrorCode::TachiomFailed,
            format!(
                "Tachiom search failed with status {}: {}{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    let rows = parse_tachiom_results(&results_path)?;
    let _ = std::fs::remove_dir_all(&work_dir);
    let mut retrieved = Vec::new();
    for row in rows.into_iter().take(options.candidate_top_k) {
        if let Some(document) = document_for_tachiom_id(&docs.documents, &row.doc_id) {
            retrieved.push(RetrievedDocument {
                concept: document.concept.clone(),
                token_vectors: document.token_vectors.clone(),
                tachiom_maxsim: row.score,
                bimaxsim: row.score,
            });
        }
    }
    rerank_retrieved(query_vectors, retrieved, options)
}

fn tachiom_engine(index_dir: &Path) -> Result<String> {
    let manifest: ArtifactManifest =
        serde_json::from_slice(&std::fs::read(index_dir.join("manifest.json"))?)?;
    Ok(manifest
        .extra
        .and_then(|extra| {
            extra
                .get("retrieval")
                .and_then(|retrieval| retrieval.get("engine"))
                .and_then(|engine| engine.as_str())
                .map(ToString::to_string)
        })
        .unwrap_or_default())
}

fn load_doc_embedding_sidecar(index_dir: &Path) -> Result<TachiomFixtureIndex> {
    let doc_embedding_dir = index_dir
        .parent()
        .ok_or_else(|| UsagiError::bad_request("Tachiom index directory has no parent"))?
        .join("doc_embeddings");
    let path = doc_embedding_dir.join("documents.json");
    serde_json::from_slice(&std::fs::read(&path)?).map_err(|err| {
        UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "THIRAWAT document embedding sidecar {} is not valid JSON: {err}",
                path.display()
            ),
        )
    })
}

#[derive(Debug)]
struct TachiomResultRow {
    doc_id: String,
    rank: usize,
    score: f32,
}

fn parse_tachiom_results(path: &Path) -> Result<Vec<TachiomResultRow>> {
    let text = std::fs::read_to_string(path)?;
    let mut rows = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(UsagiError::new(
                ErrorCode::TachiomFailed,
                format!(
                    "Tachiom result line {} must have 4 tab-separated fields",
                    line_index + 1
                ),
            ));
        }
        rows.push(TachiomResultRow {
            doc_id: fields[1].to_string(),
            rank: fields[2].parse::<usize>().map_err(|err| {
                UsagiError::new(
                    ErrorCode::TachiomFailed,
                    format!(
                        "Tachiom result rank is invalid on line {}: {err}",
                        line_index + 1
                    ),
                )
            })?,
            score: fields[3].parse::<f32>().map_err(|err| {
                UsagiError::new(
                    ErrorCode::TachiomFailed,
                    format!(
                        "Tachiom result score is invalid on line {}: {err}",
                        line_index + 1
                    ),
                )
            })?,
        });
    }
    rows.sort_by(|left, right| left.rank.cmp(&right.rank));
    Ok(rows)
}

fn document_for_tachiom_id<'a>(
    documents: &'a [TachiomFixtureDocument],
    doc_id: &str,
) -> Option<&'a TachiomFixtureDocument> {
    doc_id
        .parse::<usize>()
        .ok()
        .and_then(|index| documents.get(index))
        .or_else(|| {
            doc_id.parse::<i64>().ok().and_then(|concept_id| {
                documents
                    .iter()
                    .find(|document| document.concept.concept_id == concept_id)
            })
        })
}

fn create_tachiom_query_work_dir() -> Result<PathBuf> {
    let root =
        PathBuf::from(std::env::var("USAGI_TEMP_DIR").unwrap_or_else(|_| "temp".to_string()));
    std::fs::create_dir_all(&root)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| UsagiError::internal(format!("system clock before Unix epoch: {err}")))?
        .as_nanos();
    let path = root.join(format!("tachiom-query-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn write_query_npy(path: &Path, query_vectors: &[Vec<f32>]) -> Result<()> {
    let token_count = query_vectors.len();
    let dimension = query_vectors
        .first()
        .map(Vec::len)
        .ok_or_else(|| UsagiError::new(ErrorCode::EmbeddingFailed, "query vectors are empty"))?;
    if dimension == 0 || query_vectors.iter().any(|vector| vector.len() != dimension) {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            "query vectors must have a consistent non-zero dimension",
        ));
    }
    let mut file = std::fs::File::create(path)?;
    write_npy_header(&mut file, "<f4", &[1, token_count, dimension])?;
    for vector in query_vectors {
        for value in vector {
            file.write_all(&value.to_le_bytes())?;
        }
    }
    Ok(())
}

fn write_npy_header(mut writer: impl Write, dtype: &str, shape: &[usize]) -> Result<()> {
    let shape = match shape {
        [one] => format!("({},)", one),
        [rows, cols] => format!("({}, {})", rows, cols),
        [depth, rows, cols] => format!("({}, {}, {})", depth, rows, cols),
        _ => {
            return Err(UsagiError::bad_request(
                "minimal NPY writer supports only 1D, 2D, and 3D arrays",
            ))
        }
    };
    let mut header = format!(
        "{{'descr': '{}', 'fortran_order': False, 'shape': {}, }}",
        dtype, shape
    );
    let preamble_len = 10;
    let padding = (16 - ((preamble_len + header.len() + 1) % 16)) % 16;
    header.extend(std::iter::repeat_n(' ', padding));
    header.push('\n');
    writer.write_all(b"\x93NUMPY")?;
    writer.write_all(&[1, 0])?;
    writer.write_all(&(header.len() as u16).to_le_bytes())?;
    writer.write_all(header.as_bytes())?;
    Ok(())
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
