use std::net::SocketAddr;
use std::path::PathBuf;

use axum::body::{to_bytes, Body};
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use usagi_artifacts::job_results::job_results_response;
use usagi_catalog::store::CatalogStore;
use usagi_common::error::{ErrorCode, ErrorEnvelope, UsagiError};
use usagi_common::http::{
    api_body_limit_bytes, api_key_is_authorized, error_envelope_body, is_public_probe_path,
    API_KEY_HEADER,
};
use usagi_common::request::{generate_request_id, REQUEST_ID_HEADER};
use usagi_contracts::catalog::{
    BatchConceptsRequest, BatchConceptsResponse, CatalogBuildJobRequest, CatalogScope,
    CatalogStatus, CatalogStatusDetail, ConceptResponse, Provenance,
};
use usagi_contracts::jobs::{JobCreateResponse, JobKind};
use usagi_jobs::store::{CreateJob, JobStore};

#[derive(Clone)]
struct AppState {
    catalog_db_path: PathBuf,
    catalog_dir: PathBuf,
    jobs: JobStore,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let app = app_from_env()?;
    let addr: SocketAddr = std::env::var("CATALOG_API_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8788".to_string())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn app_from_env() -> anyhow::Result<Router> {
    let catalog_db_path = PathBuf::from(
        std::env::var("CATALOG_DB_PATH")
            .unwrap_or_else(|_| "data/catalog/catalog.sqlite".to_string()),
    );
    let catalog_dir = catalog_db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/catalog"));
    let jobs_path = PathBuf::from(
        std::env::var("JOBS_DB_PATH").unwrap_or_else(|_| "data/jobs/jobs.sqlite".to_string()),
    );
    let jobs = JobStore::open(jobs_path)?;
    jobs.migrate()?;
    Ok(router(AppState {
        catalog_db_path,
        catalog_dir,
        jobs,
    }))
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/catalog/health", get(health))
        .route("/catalog/status", get(status))
        .route("/catalog/build-job", post(build_job))
        .route("/catalog/concepts/{concept_id}", get(get_concept))
        .route("/catalog/concepts/batch", post(batch_concepts))
        .route("/catalog/concepts/{concept_id}/ancestors", get(ancestors))
        .route(
            "/catalog/concepts/{concept_id}/descendants",
            get(descendants),
        )
        .route(
            "/catalog/concepts/{concept_id}/relationships",
            get(relationships),
        )
        .route("/catalog/domains", get(reference_table_domains))
        .route("/catalog/vocabularies", get(reference_table_vocabularies))
        .route(
            "/catalog/concept-classes",
            get(reference_table_concept_classes),
        )
        .route("/jobs/{id}", get(job_status))
        .route("/jobs/{id}/events", get(job_events))
        .route("/jobs/{id}/results", get(job_results))
        .route("/jobs/{id}/cancel", post(job_cancel))
        .route("/jobs/{id}/retry", post(job_retry))
        .layer(DefaultBodyLimit::max(api_body_limit_bytes()))
        .layer(from_fn(request_id_middleware))
        .with_state(state)
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
        StatusCode::SERVICE_UNAVAILABLE => ErrorCode::CatalogNotReady,
        _ => ErrorCode::InternalError,
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok", "service": "catalog-api", "api_version": "0.1.0"}))
}

async fn status(State(state): State<AppState>) -> ApiResult<Json<CatalogStatus>> {
    if !state.catalog_db_path.exists() {
        return Ok(Json(CatalogStatus {
            status: "not_configured".to_string(),
            api_version: "0.1.0".to_string(),
            catalog: None,
            message: Some("Catalog has not been built".to_string()),
        }));
    }
    let store = CatalogStore::open(&state.catalog_db_path)?;
    let concept_count = store.concept_count()?;
    Ok(Json(CatalogStatus {
        status: "ready".to_string(),
        api_version: "0.1.0".to_string(),
        catalog: Some(CatalogStatusDetail {
            artifact_id: "local-catalog-standard-v1".to_string(),
            vocabulary_version: "unknown".to_string(),
            concept_count,
            scope: CatalogScope {
                standard_concept: "S".to_string(),
                invalid_reason: None,
            },
            manifest_path: state
                .catalog_dir
                .join("manifest.json")
                .display()
                .to_string(),
        }),
        message: None,
    }))
}

async fn build_job(
    State(state): State<AppState>,
    Json(payload): Json<CatalogBuildJobRequest>,
) -> ApiResult<Json<JobCreateResponse>> {
    let job = state.jobs.create_job(CreateJob {
        kind: JobKind::CatalogBuild,
        queue: "catalog".to_string(),
        idempotency_key: payload.idempotency_key.clone(),
        input: serde_json::to_value(&payload)?,
        total: 0,
    })?;

    Ok(Json(JobCreateResponse {
        job_id: job.id.clone(),
        state: job.state,
        status_url: format!("/jobs/{}", job.id),
        result_url: format!("/jobs/{}/results", job.id),
    }))
}

async fn get_concept(
    State(state): State<AppState>,
    Path(concept_id): Path<i64>,
) -> ApiResult<Json<ConceptResponse>> {
    let store = CatalogStore::open(state.catalog_db_path)?;
    let concept = store
        .get_concept(concept_id)?
        .ok_or_else(|| UsagiError::not_found("concept not found"))?;
    Ok(Json(ConceptResponse {
        concept,
        provenance: Provenance {
            catalog_artifact_id: Some("local-catalog-standard-v1".to_string()),
            model_artifact_id: None,
            index_artifact_id: None,
        },
    }))
}

async fn batch_concepts(
    State(state): State<AppState>,
    Json(payload): Json<BatchConceptsRequest>,
) -> ApiResult<Json<BatchConceptsResponse>> {
    let store = CatalogStore::open(state.catalog_db_path)?;
    let (concepts, missing) = store.batch_concepts(&payload.concept_ids)?;
    Ok(Json(BatchConceptsResponse { concepts, missing }))
}

#[derive(Debug, Deserialize)]
struct HierarchyQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
    #[serde(default = "default_min_level")]
    min_level: i64,
    max_level: Option<i64>,
}

async fn ancestors(
    State(state): State<AppState>,
    Path(concept_id): Path<i64>,
    Query(query): Query<HierarchyQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = CatalogStore::open(state.catalog_db_path)?;
    let items = store.ancestors(
        concept_id,
        query.limit,
        query.offset,
        query.min_level,
        query.max_level,
    )?;
    Ok(Json(
        json!({"concept_id": concept_id, "direction": "ancestors", "items": items}),
    ))
}

async fn descendants(
    State(state): State<AppState>,
    Path(concept_id): Path<i64>,
    Query(query): Query<HierarchyQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = CatalogStore::open(state.catalog_db_path)?;
    let items = store.descendants(
        concept_id,
        query.limit,
        query.offset,
        query.min_level,
        query.max_level,
    )?;
    Ok(Json(
        json!({"concept_id": concept_id, "direction": "descendants", "items": items}),
    ))
}

#[derive(Debug, Deserialize)]
struct RelationshipQuery {
    relationship_id: Option<String>,
    #[serde(default = "default_direction")]
    direction: String,
}

async fn relationships(
    State(state): State<AppState>,
    Path(concept_id): Path<i64>,
    Query(query): Query<RelationshipQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = CatalogStore::open(state.catalog_db_path)?;
    let items = store.relationships(
        concept_id,
        query.relationship_id.as_deref(),
        &query.direction,
    )?;
    Ok(Json(
        json!({"concept_id": concept_id, "relationships": items}),
    ))
}

async fn reference_table_domains(
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = CatalogStore::open(state.catalog_db_path)?;
    Ok(Json(json!({"domains": store.domains()?})))
}

async fn reference_table_vocabularies(
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = CatalogStore::open(state.catalog_db_path)?;
    Ok(Json(json!({"vocabularies": store.vocabularies()?})))
}

async fn reference_table_concept_classes(
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = CatalogStore::open(state.catalog_db_path)?;
    Ok(Json(json!({"concept_classes": store.concept_classes()?})))
}

async fn job_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let job = state
        .jobs
        .get(&id)?
        .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))?;
    Ok(Json(serde_json::to_value(job)?))
}

async fn job_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(
        json!({"job_id": id, "events": state.jobs.events(&id)?}),
    ))
}

async fn job_results(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let job = state
        .jobs
        .get(&id)?
        .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))?;
    Ok(Json(job_results_response(
        &id,
        job.state,
        state.jobs.result_json(&id)?,
        state.jobs.error_json(&id)?,
        job.artifact_path,
    )?))
}

async fn job_cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::to_value(state.jobs.cancel(&id)?)?))
}

async fn job_retry(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::to_value(state.jobs.retry(&id)?)?))
}

fn default_limit() -> i64 {
    100
}

fn default_min_level() -> i64 {
    1
}

fn default_direction() -> String {
    "both".to_string()
}

type ApiResult<T> = Result<T, ApiError>;

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
