use std::net::SocketAddr;
use std::path::PathBuf;

use axum::body::{to_bytes, Body};
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use usagi_artifacts::job_results::{job_results_download_response, job_results_response};
use usagi_common::error::{ErrorCode, ErrorEnvelope, UsagiError};
use usagi_common::http::{
    accepts_jsonl, api_body_limit_bytes, api_key_is_authorized,
    api_production_boot_errors_from_env, error_envelope_body, is_public_probe_path, API_KEY_HEADER,
};
use usagi_common::request::{generate_request_id, REQUEST_ID_HEADER};
use usagi_contracts::catalog::Provenance;
use usagi_contracts::jobs::{JobCreateResponse, JobKind};
use usagi_contracts::search::{
    SearchBatchItemResponse, SearchBatchRequest, SearchBatchResponse, SearchConceptsRequest,
    SearchConceptsResponse, SearchExplainRequest, SearchExplainResponse, SearchResult,
};
use usagi_embed::xlm_roberta::{encode_sapbert_cls, XlmRobertaEncodeOptions};
use usagi_jobs::store::{CreateJob, JobStore};
use usagi_search::dense_index::{
    search_sapbert_dense, search_sapbert_dense_from_precomputed_query, DenseSearchOptions,
};
use usagi_search::hybrid::{fuse_rrf, RrfOptions};
use usagi_search::sapbert_artifact::{validate_sapbert_artifact, SapbertArtifactPaths};
use usagi_search::tantivy_index::{
    provenance, search_tantivy, SearchFilters, TantivySearchOptions,
};

#[derive(Clone)]
struct AppState {
    catalog_db_path: PathBuf,
    tantivy_index_dir: PathBuf,
    sapbert_index_dir: PathBuf,
    sapbert_model_dir: PathBuf,
    sapbert_max_length: usize,
    sapbert_query_embeddings_path: Option<PathBuf>,
    jobs: JobStore,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let state = state_from_env()?;
    let app = Router::new()
        .route("/search/health", get(health))
        .route("/search/status", get(status))
        .route("/search/tantivy/build-job", post(tantivy_build_job))
        .route("/search/sapbert/build-job", post(sapbert_build_job))
        .route("/search/concepts", post(search_concepts))
        .route("/search/batch", post(search_batch))
        .route("/search/explain", post(search_explain))
        .route("/jobs/{id}", get(job_status))
        .route("/jobs/{id}/events", get(job_events))
        .route("/jobs/{id}/results", get(job_results))
        .route("/jobs/{id}/cancel", post(job_cancel))
        .route("/jobs/{id}/retry", post(job_retry))
        .layer(DefaultBodyLimit::max(api_body_limit_bytes()))
        .layer(from_fn(request_id_middleware));
    let app = app.with_state(state);
    let addr: SocketAddr = std::env::var("SEARCH_API_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8789".to_string())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn request_id_middleware(request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
        .unwrap_or_else(generate_request_id);
    if !is_authorized_request(&request) {
        let mut response = (
            StatusCode::UNAUTHORIZED,
            Json(ErrorEnvelope::from(
                UsagiError::new(ErrorCode::Unauthorized, "missing or invalid API key")
                    .with_request_id(&request_id),
            )),
        )
            .into_response();
        attach_request_id_header(&mut response, &request_id);
        return response;
    }
    let mut response = rewrite_error_request_id(next.run(request).await, &request_id).await;
    attach_request_id_header(&mut response, &request_id);
    response
}

fn is_authorized_request(request: &Request) -> bool {
    if is_public_probe_path(request.uri().path()) {
        return true;
    }
    let configured_keys = std::env::var("USAGI_API_KEYS").unwrap_or_default();
    let x_api_key = request
        .headers()
        .get(API_KEY_HEADER)
        .and_then(|value| value.to_str().ok());
    let authorization = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    api_key_is_authorized(&configured_keys, x_api_key, authorization)
}

fn attach_request_id_header(response: &mut Response, request_id: &str) {
    if let Ok(value) = HeaderValue::from_str(request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
}

async fn rewrite_error_request_id(response: Response, request_id: &str) -> Response {
    if !response.status().is_client_error() && !response.status().is_server_error() {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match to_bytes(body, 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorEnvelope::from(
                    UsagiError::internal(format!("failed to read error response body: {err}"))
                        .with_request_id(request_id),
                )),
            )
                .into_response();
        }
    };
    let body_bytes = error_envelope_body(&bytes, request_id, fallback_error_code(parts.status));
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Response::from_parts(parts, Body::from(body_bytes))
}

fn fallback_error_code(status: StatusCode) -> ErrorCode {
    match status {
        StatusCode::BAD_REQUEST
        | StatusCode::UNPROCESSABLE_ENTITY
        | StatusCode::UNSUPPORTED_MEDIA_TYPE
        | StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::BadRequest,
        StatusCode::NOT_FOUND => ErrorCode::NotFound,
        StatusCode::SERVICE_UNAVAILABLE => ErrorCode::IndexNotReady,
        _ => ErrorCode::InternalError,
    }
}

fn state_from_env() -> anyhow::Result<AppState> {
    require_production_boot_config()?;
    let catalog_db_path = PathBuf::from(
        std::env::var("CATALOG_DB_PATH")
            .unwrap_or_else(|_| "data/catalog/catalog.sqlite".to_string()),
    );
    let tantivy_index_dir = PathBuf::from(
        std::env::var("TANTIVY_INDEX_DIR")
            .unwrap_or_else(|_| "data/search/tantivy/index".to_string()),
    );
    let sapbert_index_dir = PathBuf::from(
        std::env::var("SAPBERT_INDEX_DIR").unwrap_or_else(|_| "data/search/sapbert".to_string()),
    );
    let jobs_path = PathBuf::from(
        std::env::var("JOBS_DB_PATH").unwrap_or_else(|_| "data/jobs/jobs.sqlite".to_string()),
    );
    let sapbert_model_dir = PathBuf::from(
        std::env::var("SAPBERT_MODEL_DIR").unwrap_or_else(|_| "data/models/sapbert".to_string()),
    );
    let sapbert_max_length = std::env::var("SAPBERT_MAX_LENGTH")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(96);
    let jobs = JobStore::open(jobs_path)?;
    jobs.migrate()?;
    Ok(AppState {
        catalog_db_path,
        tantivy_index_dir,
        sapbert_index_dir,
        sapbert_model_dir,
        sapbert_max_length,
        sapbert_query_embeddings_path: std::env::var("SAPBERT_QUERY_EMBEDDINGS_PATH")
            .ok()
            .map(PathBuf::from),
        jobs,
    })
}

fn require_production_boot_config() -> anyhow::Result<()> {
    let errors = api_production_boot_errors_from_env(&[
        "CATALOG_DB_PATH",
        "TANTIVY_INDEX_DIR",
        "SAPBERT_INDEX_DIR",
        "SAPBERT_MODEL_DIR",
        "JOB_RESULTS_DIR",
    ]);
    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", errors.join("; "))
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok", "service": "search-api", "api_version": "0.1.0"}))
}

async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let tantivy_status = if state.tantivy_index_dir.join("meta.json").exists() {
        "ready"
    } else {
        "not_configured"
    };
    let sapbert_status = if validate_sapbert_artifact(SapbertArtifactPaths {
        artifact_dir: state.sapbert_index_dir,
    })
    .is_ok()
    {
        "ready"
    } else {
        "not_configured"
    };
    Json(json!({
        "status": if tantivy_status == "ready" && sapbert_status == "ready" {
            "ready"
        } else if tantivy_status == "ready" {
            "degraded"
        } else {
            "not_configured"
        },
        "api_version": "0.1.0",
        "catalog": {
            "path": state.catalog_db_path
        },
        "indexes": {
            "tantivy": {"status": tantivy_status},
            "sapbert": {"status": sapbert_status}
        }
    }))
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct TantivyBuildJobRequest {
    idempotency_key: String,
    #[serde(default)]
    overwrite: bool,
    #[serde(default = "default_tantivy_schema_version")]
    schema_version: String,
}

async fn tantivy_build_job(
    State(state): State<AppState>,
    Json(payload): Json<TantivyBuildJobRequest>,
) -> Result<Json<JobCreateResponse>, ApiError> {
    let job = state.jobs.create_job(CreateJob {
        kind: JobKind::TantivyBuild,
        queue: "index".to_string(),
        idempotency_key: payload.idempotency_key.clone(),
        input: serde_json::to_value(payload)?,
        total: 0,
    })?;
    Ok(Json(JobCreateResponse {
        job_id: job.id.clone(),
        state: job.state,
        status_url: format!("/jobs/{}", job.id),
        result_url: format!("/jobs/{}/results", job.id),
    }))
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct SapbertBuildJobRequest {
    idempotency_key: String,
    #[serde(default)]
    overwrite: bool,
    #[serde(default = "default_sapbert_model_artifact_id")]
    model_artifact_id: String,
    #[serde(default = "default_batch_size")]
    batch_size: usize,
    #[serde(default)]
    scope: serde_json::Value,
}

async fn sapbert_build_job(
    State(state): State<AppState>,
    Json(payload): Json<SapbertBuildJobRequest>,
) -> Result<Json<JobCreateResponse>, ApiError> {
    let job = state.jobs.create_job(CreateJob {
        kind: JobKind::SapbertBuild,
        queue: "embed".to_string(),
        idempotency_key: payload.idempotency_key.clone(),
        input: serde_json::to_value(payload)?,
        total: 0,
    })?;
    Ok(Json(JobCreateResponse {
        job_id: job.id.clone(),
        state: job.state,
        status_url: format!("/jobs/{}", job.id),
        result_url: format!("/jobs/{}/results", job.id),
    }))
}

fn sapbert_query_results(
    state: &AppState,
    q: &str,
    limit: usize,
) -> Result<Vec<usagi_contracts::search::SearchResult>, ApiError> {
    if let Some(query_embeddings_path) = &state.sapbert_query_embeddings_path {
        return Ok(search_sapbert_dense_from_precomputed_query(
            DenseSearchOptions {
                artifact_dir: state.sapbert_index_dir.clone(),
                catalog_db_path: state.catalog_db_path.clone(),
                query_vector: Vec::new(),
                limit,
            },
            query_embeddings_path,
            q,
        )?);
    }
    let query_embedding = encode_sapbert_cls(
        XlmRobertaEncodeOptions {
            model_dir: state.sapbert_model_dir.clone(),
            max_length: state.sapbert_max_length,
        },
        &[q],
    )?
    .into_iter()
    .next()
    .ok_or_else(|| {
        ApiError(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            "SapBERT query encoder produced no vectors",
        ))
    })?;
    Ok(search_sapbert_dense(DenseSearchOptions {
        artifact_dir: state.sapbert_index_dir.clone(),
        catalog_db_path: state.catalog_db_path.clone(),
        query_vector: query_embedding.vector,
        limit,
    })?)
}

async fn search_concepts(
    State(state): State<AppState>,
    Json(payload): Json<SearchConceptsRequest>,
) -> Result<Json<SearchConceptsResponse>, ApiError> {
    let (results, provenance) = search_results(
        &state,
        &payload.mode,
        &payload.q,
        payload.limit,
        &payload.filters,
        &payload.hybrid,
    )?;
    Ok(Json(SearchConceptsResponse {
        query: payload.q,
        mode: payload.mode,
        results,
        provenance,
    }))
}

async fn search_batch(
    State(state): State<AppState>,
    Json(payload): Json<SearchBatchRequest>,
) -> Result<Json<SearchBatchResponse>, ApiError> {
    let mut items = Vec::with_capacity(payload.items.len());
    let mut response_provenance = None;
    for item in payload.items {
        let (results, provenance) = search_results(
            &state,
            &payload.mode,
            &item.q,
            payload.limit_per_item,
            &item.filters,
            &serde_json::Value::Null,
        )?;
        response_provenance.get_or_insert(provenance);
        items.push(SearchBatchItemResponse {
            id: item.id,
            results,
        });
    }
    Ok(Json(SearchBatchResponse {
        mode: payload.mode,
        items,
        provenance: response_provenance.unwrap_or_else(|| {
            provenance(
                "local-catalog-standard-v1".to_string(),
                "local-empty-batch-v1".to_string(),
            )
        }),
    }))
}

fn search_results(
    state: &AppState,
    mode: &str,
    q: &str,
    limit: usize,
    filters: &serde_json::Value,
    hybrid: &serde_json::Value,
) -> Result<(Vec<SearchResult>, Provenance), ApiError> {
    match mode {
        "sapbert_cls" => {
            validate_sapbert_artifact(SapbertArtifactPaths {
                artifact_dir: state.sapbert_index_dir.clone(),
            })?;
            Ok((
                sapbert_query_results(state, q, limit)?,
                provenance(
                    "local-catalog-standard-v1".to_string(),
                    "local-sapbert-cls-v1".to_string(),
                ),
            ))
        }
        "hybrid_rrf" => {
            validate_sapbert_artifact(SapbertArtifactPaths {
                artifact_dir: state.sapbert_index_dir.clone(),
            })?;
            let dense = sapbert_query_results(
                state,
                q,
                hybrid
                    .get("sapbert_top_k")
                    .and_then(|value| value.as_u64())
                    .map(|value| value as usize)
                    .unwrap_or(100),
            )?;
            let lexical = search_tantivy(TantivySearchOptions {
                index_dir: state.tantivy_index_dir.clone(),
                q: q.to_string(),
                limit: hybrid
                    .get("lexical_top_k")
                    .and_then(|value| value.as_u64())
                    .map(|value| value as usize)
                    .unwrap_or(100),
                filters: filters_from_json(filters),
            })?;
            Ok((
                fuse_rrf(
                    lexical,
                    dense,
                    RrfOptions {
                        rrf_k: hybrid
                            .get("rrf_k")
                            .and_then(|value| value.as_f64())
                            .unwrap_or(60.0),
                        limit,
                    },
                ),
                provenance(
                    "local-catalog-standard-v1".to_string(),
                    "local-hybrid-rrf-v1".to_string(),
                ),
            ))
        }
        "lexical_tantivy" => Ok((
            search_tantivy(TantivySearchOptions {
                index_dir: state.tantivy_index_dir.clone(),
                q: q.to_string(),
                limit,
                filters: filters_from_json(filters),
            })?,
            provenance(
                "local-catalog-standard-v1".to_string(),
                "local-tantivy-v1".to_string(),
            ),
        )),
        _ => Err(ApiError(UsagiError::bad_request("unsupported search mode"))),
    }
}

async fn search_explain(
    State(state): State<AppState>,
    Json(payload): Json<SearchExplainRequest>,
) -> Result<Json<SearchExplainResponse>, ApiError> {
    let (results, _) = search_results(
        &state,
        &payload.mode,
        &payload.q,
        100,
        &serde_json::Value::Null,
        &serde_json::Value::Null,
    )?;
    let result = results
        .into_iter()
        .find(|item| item.concept.concept_id == payload.concept_id)
        .ok_or_else(|| UsagiError::not_found("concept was not found in search results"))?;
    Ok(Json(SearchExplainResponse {
        query: payload.q,
        concept_id: payload.concept_id,
        explanation: explain_search_result(&payload.mode, &result),
    }))
}

fn explain_search_result(mode: &str, result: &SearchResult) -> serde_json::Value {
    match mode {
        "sapbert_cls" => json!({
            "sapbert": {
                "rank": result.rank,
                "score": result.scores.get("sapbert").cloned().unwrap_or(serde_json::Value::Null)
            }
        }),
        "hybrid_rrf" => json!({
            "tantivy": {
                "rank": result.component_ranks.get("tantivy").cloned().unwrap_or(serde_json::Value::Null)
            },
            "sapbert": {
                "rank": result.component_ranks.get("sapbert").cloned().unwrap_or(serde_json::Value::Null)
            },
            "hybrid_rrf": {
                "rank": result.rank,
                "score": result.scores.get("rrf").cloned().unwrap_or(serde_json::Value::Null),
                "rrf_k": 60
            }
        }),
        _ => json!({
            "tantivy": {
                "rank": result.rank,
                "score": result.scores.get("tantivy").cloned().unwrap_or(serde_json::Value::Null),
                "matched_fields": ["concept_name", "search_blob"]
            }
        }),
    }
}

async fn job_status(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let job = state
        .jobs
        .get(&id)?
        .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))?;
    Ok(Json(serde_json::to_value(job)?))
}

async fn job_events(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        json!({"job_id": id, "events": state.jobs.events(&id)?}),
    ))
}

async fn job_results(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let job = state
        .jobs
        .get(&id)?
        .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))?;
    if accepts_jsonl(
        headers
            .get(header::ACCEPT)
            .and_then(|value| value.to_str().ok()),
    ) {
        if let Some(path) = job.artifact_path.clone() {
            let download = job_results_download_response(path)?;
            let mut response = Body::from(download.body).into_response();
            let content_type = HeaderValue::from_str(&download.content_type)
                .map_err(|err| UsagiError::internal(err.to_string()))?;
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, content_type);
            return Ok(response);
        }
    }
    Ok(Json(job_results_response(
        &id,
        job.state,
        state.jobs.result_json(&id)?,
        state.jobs.error_json(&id)?,
        job.artifact_path,
    )?)
    .into_response())
}

async fn job_cancel(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::to_value(state.jobs.cancel(&id)?)?))
}

async fn job_retry(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::to_value(state.jobs.retry(&id)?)?))
}

struct ApiError(UsagiError);

impl From<UsagiError> for ApiError {
    fn from(value: UsagiError) -> Self {
        Self(value)
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(value: serde_json::Error) -> Self {
        Self(UsagiError::internal(value.to_string()))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0.code() {
            ErrorCode::BadRequest => StatusCode::BAD_REQUEST,
            ErrorCode::NotFound | ErrorCode::JobNotFound => StatusCode::NOT_FOUND,
            ErrorCode::CatalogNotReady | ErrorCode::IndexNotReady | ErrorCode::ModelNotReady => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(ErrorEnvelope::from(self.0))).into_response()
    }
}

fn filters_from_json(value: &serde_json::Value) -> SearchFilters {
    SearchFilters {
        domain_id: string_array(value, "domain_id"),
        vocabulary_id: string_array(value, "vocabulary_id"),
        concept_class_id: string_array(value, "concept_class_id"),
    }
}

fn string_array(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn default_tantivy_schema_version() -> String {
    "usagi-tantivy-v1".to_string()
}

fn default_sapbert_model_artifact_id() -> String {
    "sapbert-xlmr-merged-v1".to_string()
}

fn default_batch_size() -> usize {
    32
}
