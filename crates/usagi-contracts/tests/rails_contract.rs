use std::path::Path;

use serde_json::Value;
use usagi_contracts::jobs::JobDto;
use usagi_contracts::jobs::{JobCreateResponse, JobState};
use usagi_contracts::mapper::{
    MapperDrugBatchJobRequest, MapperDrugQueryRequest, MapperDrugQueryResponse,
};
use usagi_contracts::search::{SearchConceptsRequest, SearchConceptsResponse};

#[test]
fn job_create_response_exposes_status_and_result_urls_for_rails_mirrors() {
    let response: JobCreateResponse = serde_json::from_value(serde_json::json!({
        "job_id": "job_map_abc",
        "state": "queued",
        "status_url": "/jobs/job_map_abc",
        "result_url": "/jobs/job_map_abc/results"
    }))
    .unwrap();

    assert_eq!(response.job_id, "job_map_abc");
    assert_eq!(response.state, JobState::Queued);
    assert_eq!(response.status_url, "/jobs/job_map_abc");
    assert_eq!(response.result_url, "/jobs/job_map_abc/results");
}

#[test]
fn rails_contract_fixtures_cover_minimum_api_checkpoint() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/contracts");

    let search_request: SearchConceptsRequest =
        read_json(root.join("search_concepts.hybrid_rrf.request.json"));
    assert_eq!(search_request.mode, "hybrid_rrf");
    assert_eq!(search_request.limit, 20);

    let search_response: SearchConceptsResponse =
        read_json(root.join("search_concepts.hybrid_rrf.response.json"));
    assert_eq!(search_response.results[0].concept.standard_concept, "S");
    assert_eq!(
        search_response.provenance.catalog_artifact_id.as_deref(),
        Some("local-catalog-standard-v1")
    );

    let mapper_query: MapperDrugQueryRequest =
        read_json(root.join("mapper_drugs_query.request.json"));
    assert_eq!(mapper_query.mode, "thirawat_tachiom");
    assert_eq!(mapper_query.candidate_top_k, 200);

    let mapper_response: MapperDrugQueryResponse =
        read_json(root.join("mapper_drugs_query.response.json"));
    assert_eq!(mapper_response.candidates[0].concept.domain_id, "Drug");
    assert_eq!(
        mapper_response.provenance.model_artifact_id.as_deref(),
        Some("sidataplus/THIRAWAT-SapBERT")
    );

    let mapper_batch_job: MapperDrugBatchJobRequest =
        read_json(root.join("mapper_drugs_batch_job.request.json"));
    assert_eq!(mapper_batch_job.mode, "thirawat_tachiom");
    assert_eq!(mapper_batch_job.items.len(), 2);

    let batch_job_response: JobCreateResponse =
        read_json(root.join("mapper_drugs_batch_job.response.json"));
    assert_eq!(batch_job_response.status_url, "/jobs/job_map_abc");
    assert_eq!(batch_job_response.result_url, "/jobs/job_map_abc/results");

    let jobs_running: JobDto = read_json(root.join("jobs_status.running.json"));
    assert_eq!(jobs_running.state.as_str(), "running");
    assert_eq!(jobs_running.kind.as_str(), "mapper_drugs_batch");

    let jobs_results: Value = read_json(root.join("jobs_results.mapper_success.json"));
    assert_eq!(jobs_results["job_id"], "job_map_abc");
    assert_eq!(
        jobs_results["artifact"]["content_type"],
        "application/jsonl"
    );

    let index_not_ready: Value = read_json(root.join("error.index_not_ready.json"));
    assert_eq!(index_not_ready["error"]["code"], "INDEX_NOT_READY");
    assert_eq!(index_not_ready["error"]["request_id"], "req_contract");
}

fn read_json<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> T {
    let path = path.as_ref();
    let bytes = std::fs::read(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}
