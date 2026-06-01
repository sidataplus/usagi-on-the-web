use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use axum::body::{to_bytes, Body};
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use usagi_artifacts::job_results::{job_results_download_response, job_results_response};
use usagi_common::error::{ErrorCode, ErrorEnvelope, UsagiError};
use usagi_common::http::{
    accepts_jsonl, api_body_limit_bytes, api_key_is_authorized,
    api_production_boot_errors_from_env, error_envelope_body, is_public_probe_path,
    verify_signed_request, SignatureNonceCache, SignedAuthConfig, SignedRequest, API_KEY_HEADER,
};
use usagi_common::request::{generate_request_id, REQUEST_ID_HEADER};
use usagi_contracts::catalog::Provenance;
use usagi_contracts::jobs::{JobCreateResponse, JobKind};
use usagi_contracts::mapper::{
    MapperDrugBatchItemResponse, MapperDrugBatchJobRequest, MapperDrugBatchRequest,
    MapperDrugBatchResponse, MapperDrugExplainRequest, MapperDrugExplainResponse,
    MapperDrugQueryRequest, TachiomBuildJobRequest, ThirawatBuildEmbeddingsJobRequest,
};
use usagi_embed::xlm_roberta::{encode_projected_tokens, XlmRobertaTokenEncodeOptions};
use usagi_jobs::store::{CreateJob, JobStore};
use usagi_thirawat::artifact::{
    validate_tachiom_artifact, validate_thirawat_doc_embedding_artifact,
    validate_thirawat_model_artifact, TachiomArtifactPaths, ThirawatDocEmbeddingArtifactPaths,
    ThirawatModelArtifactPaths,
};
use usagi_thirawat::mapper::{
    map_drug_batch_from_precomputed, map_drug_explain_from_precomputed,
    map_drug_explain_with_vectors, map_drug_query_from_precomputed, map_drug_query_with_vectors,
    MapperRuntimeOptions,
};

#[derive(Clone)]
struct AppState {
    jobs: JobStore,
    thirawat_model_dir: PathBuf,
    thirawat_doc_embedding_dir: PathBuf,
    tachiom_index_dir: PathBuf,
    query_embeddings_path: Option<PathBuf>,
}

static SIGNATURE_NONCE_CACHE: OnceLock<Mutex<SignatureNonceCache>> = OnceLock::new();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let state = state_from_env()?;
    let app = Router::new()
        .route("/mapper/health", get(health))
        .route("/mapper/status", get(status))
        .route(
            "/mapper/thirawat/build-embeddings-job",
            post(thirawat_build_embeddings_job),
        )
        .route("/mapper/tachiom/build-index-job", post(tachiom_build_job))
        .route("/mapper/drugs/query", post(drug_query))
        .route("/mapper/drugs/batch", post(drug_batch))
        .route("/mapper/drugs/batch-job", post(drug_batch_job))
        .route("/mapper/drugs/explain", post(drug_explain))
        .route("/jobs/{id}", get(job_status))
        .route("/jobs/{id}/events", get(job_events))
        .route("/jobs/{id}/results", get(job_results))
        .route("/jobs/{id}/cancel", post(job_cancel))
        .route("/jobs/{id}/retry", post(job_retry))
        .layer(DefaultBodyLimit::max(api_body_limit_bytes()))
        .layer(from_fn(request_id_middleware))
        .with_state(state);
    let addr: SocketAddr = std::env::var("MAPPER_API_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8790".to_string())
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
    let request = match authorize_request(request, &request_id).await {
        Ok(request) => request,
        Err(err) => {
            let mut response =
                (StatusCode::UNAUTHORIZED, Json(ErrorEnvelope::from(err))).into_response();
            attach_request_id_header(&mut response, &request_id);
            return response;
        }
    };
    let mut response = rewrite_error_request_id(next.run(request).await, &request_id).await;
    attach_request_id_header(&mut response, &request_id);
    response
}

async fn authorize_request(request: Request, request_id: &str) -> Result<Request, UsagiError> {
    if is_public_probe_path(request.uri().path()) {
        return Ok(request);
    }
    match std::env::var("USAGI_API_AUTH_MODE")
        .unwrap_or_else(|_| "api_key".to_string())
        .as_str()
    {
        "disabled" => Ok(request),
        "signed" => authorize_signed_request(request, request_id).await,
        _ => {
            if api_key_authorized(&request) {
                Ok(request)
            } else {
                Err(
                    UsagiError::new(ErrorCode::Unauthorized, "missing or invalid API key")
                        .with_request_id(request_id),
                )
            }
        }
    }
}

async fn authorize_signed_request(
    request: Request,
    request_id: &str,
) -> Result<Request, UsagiError> {
    let secret = std::env::var("USAGI_API_SHARED_SECRET").map_err(|_| {
        UsagiError::new(
            ErrorCode::SignatureRequired,
            "USAGI_API_SHARED_SECRET is not configured",
        )
        .with_request_id(request_id)
    })?;
    let (parts, body) = request.into_parts();
    let path_with_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| parts.uri.path().to_string());
    let headers = parts
        .headers
        .iter()
        .filter_map(|(key, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (key.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect();
    let bytes = to_bytes(body, api_body_limit_bytes())
        .await
        .map_err(|err| {
            UsagiError::internal(format!("failed to read request body: {err}"))
                .with_request_id(request_id)
        })?;
    let signed_request = SignedRequest {
        method: parts.method.as_str(),
        path_with_query: &path_with_query,
        headers,
        body: &bytes,
    };
    let config = SignedAuthConfig {
        secret: &secret,
        now: chrono::Utc::now(),
        allowed_clock_skew_seconds: std::env::var("USAGI_API_ALLOWED_CLOCK_SKEW_SECONDS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(300),
        nonce_ttl_seconds: std::env::var("USAGI_API_NONCE_TTL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(300),
    };
    let nonce_cache =
        SIGNATURE_NONCE_CACHE.get_or_init(|| Mutex::new(SignatureNonceCache::default()));
    verify_signed_request(
        config,
        signed_request,
        &mut nonce_cache.lock().expect("signature nonce cache poisoned"),
    )?;
    Ok(Request::from_parts(parts, Body::from(bytes)))
}

fn api_key_authorized(request: &Request) -> bool {
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
        StatusCode::SERVICE_UNAVAILABLE => ErrorCode::ModelNotReady,
        _ => ErrorCode::InternalError,
    }
}

fn state_from_env() -> anyhow::Result<AppState> {
    require_production_boot_config()?;
    let jobs_path = PathBuf::from(
        std::env::var("JOBS_DB_PATH").unwrap_or_else(|_| "data/jobs/jobs.sqlite".to_string()),
    );
    let jobs = JobStore::open(jobs_path)?;
    jobs.migrate()?;
    let artifact_dir = PathBuf::from(
        std::env::var("THIRAWAT_ARTIFACT_DIR")
            .unwrap_or_else(|_| "data/mapper/thirawat-drug".to_string()),
    );
    Ok(AppState {
        jobs,
        thirawat_model_dir: PathBuf::from(
            std::env::var("THIRAWAT_MODEL_DIR")
                .unwrap_or_else(|_| "data/models/thirawat-sapbert".to_string()),
        ),
        thirawat_doc_embedding_dir: artifact_dir.join("doc_embeddings"),
        tachiom_index_dir: PathBuf::from(
            std::env::var("TACHIOM_INDEX_DIR")
                .unwrap_or_else(|_| artifact_dir.join("tachiom").display().to_string()),
        ),
        query_embeddings_path: std::env::var("THIRAWAT_QUERY_EMBEDDINGS_PATH")
            .ok()
            .map(PathBuf::from),
    })
}

fn require_production_boot_config() -> anyhow::Result<()> {
    let errors = api_production_boot_errors_from_env(&[
        "THIRAWAT_MODEL_DIR",
        "THIRAWAT_ARTIFACT_DIR",
        "TACHIOM_INDEX_DIR",
        "JOB_RESULTS_DIR",
    ]);
    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", errors.join("; "))
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok", "service": "mapper-api", "api_version": "0.1.0"}))
}

async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let model_status = status_label(validate_thirawat_model_artifact(
        ThirawatModelArtifactPaths {
            model_dir: state.thirawat_model_dir,
        },
    ));
    let doc_status = status_label(validate_thirawat_doc_embedding_artifact(
        ThirawatDocEmbeddingArtifactPaths {
            doc_embedding_dir: state.thirawat_doc_embedding_dir,
        },
    ));
    let tachiom_status = status_label(validate_tachiom_artifact(TachiomArtifactPaths {
        index_dir: state.tachiom_index_dir,
    }));
    let query_status = status_label(state.query_embeddings_path.as_ref().map_or(
        Err(UsagiError::new(
            ErrorCode::IndexNotReady,
            "precomputed query embeddings are not configured",
        )),
        |path| {
            if path.exists() {
                Ok(())
            } else {
                Err(UsagiError::new(
                    ErrorCode::IndexNotReady,
                    format!(
                        "precomputed query embeddings are missing {}",
                        path.display()
                    ),
                ))
            }
        },
    ));
    Json(json!({
        "status": mapper_readiness_status(model_status, doc_status, tachiom_status, query_status),
        "api_version": "0.1.0",
        "domain_support": ["Drug"],
        "query_mode": if query_status == "ready" { "precomputed" } else { "model" },
        "model": {
            "status": model_status,
            "model_id": "sidataplus/THIRAWAT-SapBERT"
        },
        "indexes": {
            "thirawat_doc_embeddings": {"status": doc_status},
            "tachiom": {"status": tachiom_status},
            "precomputed_query_embeddings": {
                "status": query_status
            }
        }
    }))
}

async fn thirawat_build_embeddings_job(
    State(state): State<AppState>,
    Json(payload): Json<ThirawatBuildEmbeddingsJobRequest>,
) -> Result<Json<JobCreateResponse>, ApiError> {
    let job = state.jobs.create_job(CreateJob {
        kind: JobKind::ThirawatDocEmbed,
        queue: "embed".to_string(),
        idempotency_key: payload.idempotency_key.clone(),
        input: serde_json::to_value(payload)?,
        total: 0,
    })?;
    Ok(Json(job_response(job)))
}

async fn tachiom_build_job(
    State(state): State<AppState>,
    Json(payload): Json<TachiomBuildJobRequest>,
) -> Result<Json<JobCreateResponse>, ApiError> {
    let job = state.jobs.create_job(CreateJob {
        kind: JobKind::TachiomBuild,
        queue: "index".to_string(),
        idempotency_key: payload.idempotency_key.clone(),
        input: serde_json::to_value(payload)?,
        total: 0,
    })?;
    Ok(Json(job_response(job)))
}

async fn drug_query(
    State(state): State<AppState>,
    Json(payload): Json<MapperDrugQueryRequest>,
) -> Result<Response, ApiError> {
    if let Some(query_embeddings_path) = &state.query_embeddings_path {
        validate_tachiom_artifact(TachiomArtifactPaths {
            index_dir: state.tachiom_index_dir.clone(),
        })?;
        let response = map_drug_query_from_precomputed(
            &payload.source_name,
            payload.source_code.as_deref(),
            query_embeddings_path,
            &state.tachiom_index_dir,
            runtime_options(
                payload.candidate_top_k,
                payload.rerank_top_n,
                payload.limit,
                &payload.post_rank,
            ),
        )?;
        return Ok(Json(response).into_response());
    }
    validate_mapper_ready(&state)?;
    let token_embedding = encode_thirawat_query(&state, &payload.source_name)?;
    let response = map_drug_query_with_vectors(
        &payload.source_name,
        payload.source_code.as_deref(),
        token_embedding.vectors,
        &state.tachiom_index_dir,
        runtime_options(
            payload.candidate_top_k,
            payload.rerank_top_n,
            payload.limit,
            &payload.post_rank,
        ),
    )?;
    Ok(Json(response).into_response())
}

async fn drug_batch(
    State(state): State<AppState>,
    Json(payload): Json<MapperDrugBatchRequest>,
) -> Result<Response, ApiError> {
    if let Some(query_embeddings_path) = &state.query_embeddings_path {
        validate_tachiom_artifact(TachiomArtifactPaths {
            index_dir: state.tachiom_index_dir.clone(),
        })?;
        let response = map_drug_batch_from_precomputed(
            payload,
            query_embeddings_path,
            &state.tachiom_index_dir,
        )?;
        return Ok(Json(response).into_response());
    }
    validate_mapper_ready(&state)?;
    let mode = payload.mode;
    let mut items = Vec::with_capacity(payload.items.len());
    for item in payload.items {
        let item_id = item.id;
        let response = encode_thirawat_query(&state, &item.source_name).and_then(|embedding| {
            map_drug_query_with_vectors(
                &item.source_name,
                item.source_code.as_deref(),
                embedding.vectors,
                &state.tachiom_index_dir,
                MapperRuntimeOptions {
                    candidate_top_k: payload.candidate_top_k,
                    rerank_top_n: payload.rerank_top_n,
                    limit: payload.limit,
                    ..MapperRuntimeOptions::default()
                },
            )
        });
        match response {
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
    Ok(Json(MapperDrugBatchResponse {
        mode,
        items,
        provenance: Provenance {
            catalog_artifact_id: None,
            model_artifact_id: Some("sidataplus/THIRAWAT-SapBERT".to_string()),
            index_artifact_id: Some(
                state
                    .tachiom_index_dir
                    .join("manifest.json")
                    .display()
                    .to_string(),
            ),
        },
    })
    .into_response())
}

async fn drug_batch_job(
    State(state): State<AppState>,
    Json(payload): Json<MapperDrugBatchJobRequest>,
) -> Result<Json<JobCreateResponse>, ApiError> {
    let total = payload.items.len() as i64;
    let job = state.jobs.create_job(CreateJob {
        kind: JobKind::MapperDrugsBatch,
        queue: "map".to_string(),
        idempotency_key: payload.idempotency_key.clone(),
        input: serde_json::to_value(payload)?,
        total,
    })?;
    Ok(Json(job_response(job)))
}

async fn drug_explain(
    State(state): State<AppState>,
    Json(payload): Json<MapperDrugExplainRequest>,
) -> Result<Json<MapperDrugExplainResponse>, ApiError> {
    if let Some(query_embeddings_path) = &state.query_embeddings_path {
        validate_tachiom_artifact(TachiomArtifactPaths {
            index_dir: state.tachiom_index_dir.clone(),
        })?;
        let response = map_drug_explain_from_precomputed(
            &payload.source_name,
            payload.source_code.as_deref(),
            payload.concept_id,
            query_embeddings_path,
            &state.tachiom_index_dir,
            MapperRuntimeOptions::default(),
        )?;
        return Ok(Json(response));
    }
    validate_mapper_ready(&state)?;
    let token_embedding = encode_thirawat_query(&state, &payload.source_name)?;
    Ok(Json(map_drug_explain_with_vectors(
        &payload.source_name,
        payload.source_code.as_deref(),
        payload.concept_id,
        token_embedding.vectors,
        &state.tachiom_index_dir,
        MapperRuntimeOptions::default(),
    )?))
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

fn validate_mapper_ready(state: &AppState) -> Result<(), UsagiError> {
    validate_thirawat_model_artifact(ThirawatModelArtifactPaths {
        model_dir: state.thirawat_model_dir.clone(),
    })?;
    validate_thirawat_doc_embedding_artifact(ThirawatDocEmbeddingArtifactPaths {
        doc_embedding_dir: state.thirawat_doc_embedding_dir.clone(),
    })?;
    validate_tachiom_artifact(TachiomArtifactPaths {
        index_dir: state.tachiom_index_dir.clone(),
    })?;
    Ok(())
}

fn encode_thirawat_query(
    state: &AppState,
    source_name: &str,
) -> Result<usagi_embed::types::TokenEmbedding, UsagiError> {
    encode_projected_tokens(
        XlmRobertaTokenEncodeOptions {
            model_dir: state.thirawat_model_dir.clone(),
            projection_safetensors: state
                .thirawat_model_dir
                .join("colbert_projection.safetensors"),
            max_length: 96,
            output_dim: 128,
        },
        &[source_name],
    )?
    .into_iter()
    .next()
    .ok_or_else(|| {
        UsagiError::new(
            ErrorCode::EmbeddingFailed,
            "THIRAWAT query encoder produced no vectors",
        )
    })
}

fn error_value(err: &UsagiError) -> serde_json::Value {
    serde_json::json!({
        "code": err.code().as_str(),
        "message": err.message()
    })
}

fn runtime_options(
    candidate_top_k: usize,
    rerank_top_n: usize,
    limit: usize,
    post_rank: &serde_json::Value,
) -> MapperRuntimeOptions {
    MapperRuntimeOptions {
        candidate_top_k,
        rerank_top_n,
        limit,
        epsilon: post_rank
            .get("epsilon")
            .and_then(|value| value.as_f64())
            .map(|value| value as f32)
            .unwrap_or(0.01),
        tiebreak_top_n: post_rank
            .get("top_n")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(100),
    }
}

fn job_response(job: usagi_contracts::jobs::JobDto) -> JobCreateResponse {
    JobCreateResponse {
        job_id: job.id.clone(),
        state: job.state,
        status_url: format!("/jobs/{}", job.id),
        result_url: format!("/jobs/{}/results", job.id),
    }
}

fn status_label(result: Result<(), UsagiError>) -> &'static str {
    if result.is_ok() {
        "ready"
    } else {
        "not_configured"
    }
}

fn mapper_readiness_status(
    model_status: &str,
    doc_status: &str,
    tachiom_status: &str,
    query_status: &str,
) -> &'static str {
    if doc_status == "ready"
        && tachiom_status == "ready"
        && (model_status == "ready" || query_status == "ready")
    {
        "ready"
    } else {
        "not_configured"
    }
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

#[cfg(test)]
mod tests {
    use super::mapper_readiness_status;

    #[test]
    fn mapper_is_ready_with_full_model_artifacts() {
        assert_eq!(
            mapper_readiness_status("ready", "ready", "ready", "not_configured"),
            "ready"
        );
    }

    #[test]
    fn mapper_is_ready_with_precomputed_query_artifacts() {
        assert_eq!(
            mapper_readiness_status("not_configured", "ready", "ready", "ready"),
            "ready"
        );
    }

    #[test]
    fn mapper_is_not_ready_without_retrieval_artifacts() {
        assert_eq!(
            mapper_readiness_status("ready", "ready", "not_configured", "ready"),
            "not_configured"
        );
    }
}
