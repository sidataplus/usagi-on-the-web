use serde_json::json;
use usagi_contracts::jobs::{JobKind, JobState};
use usagi_jobs::store::{CreateJob, JobStore};

#[test]
fn duplicate_idempotency_key_returns_existing_job() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let first = store
        .create_job(CreateJob {
            kind: JobKind::CatalogBuild,
            queue: "catalog".to_string(),
            idempotency_key: "catalog-mini-v1".to_string(),
            input: json!({"athena_dir": "/fixtures/athena-mini"}),
            total: 0,
        })
        .unwrap();
    let second = store
        .create_job(CreateJob {
            kind: JobKind::CatalogBuild,
            queue: "catalog".to_string(),
            idempotency_key: "catalog-mini-v1".to_string(),
            input: json!({"athena_dir": "/fixtures/athena-mini"}),
            total: 0,
        })
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(second.state, JobState::Queued);
}

#[test]
fn claim_next_job_moves_one_queued_job_to_running() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let created = store
        .create_job(CreateJob {
            kind: JobKind::CatalogBuild,
            queue: "catalog".to_string(),
            idempotency_key: "catalog-mini-v1".to_string(),
            input: json!({}),
            total: 0,
        })
        .unwrap();

    let claimed = store.claim_next(&["catalog"]).unwrap().unwrap();
    assert_eq!(claimed.id, created.id);
    assert_eq!(claimed.state, JobState::Running);
    assert!(claimed.started_at.is_some());

    assert!(store.claim_next(&["catalog"]).unwrap().is_none());
}

#[test]
fn partial_failure_marks_succeeded_with_errors() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::MapperDrugsBatch,
            queue: "map".to_string(),
            idempotency_key: "map-v1".to_string(),
            input: json!({}),
            total: 2,
        })
        .unwrap();

    store
        .record_item_failure(&job.id, "src_bad", json!({"code": "BAD_REQUEST"}))
        .unwrap();
    let done = store.finish_itemized(&job.id, 2).unwrap();

    assert_eq!(done.state, JobState::SucceededWithErrors);
    assert_eq!(done.processed, 2);
    assert_eq!(done.failed, 1);
}

#[test]
fn job_items_store_success_input_and_result_json() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("jobs.sqlite");
    let store = JobStore::open(&db_path).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::MapperDrugsBatch,
            queue: "map".to_string(),
            idempotency_key: "map-item-success-v1".to_string(),
            input: json!({}),
            total: 1,
        })
        .unwrap();

    store
        .record_item_success(
            &job.id,
            "src_good",
            json!({"source_name": "aspirin 81 mg tablet"}),
            json!({"candidates": [{"concept": {"concept_id": 111}}]}),
        )
        .unwrap();

    let conn = rusqlite::Connection::open(db_path).unwrap();
    let (state, input_json, result_json): (String, String, String) = conn
        .query_row(
            "SELECT state, input_json, result_json FROM job_items WHERE job_id = ?1 AND item_key = ?2",
            (&job.id, "src_good"),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let input: serde_json::Value = serde_json::from_str(&input_json).unwrap();
    let result: serde_json::Value = serde_json::from_str(&result_json).unwrap();

    assert_eq!(state, "succeeded");
    assert_eq!(input["source_name"], "aspirin 81 mg tablet");
    assert_eq!(result["candidates"][0]["concept"]["concept_id"], 111);
}

#[test]
fn job_items_store_failure_input_and_error_json() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("jobs.sqlite");
    let store = JobStore::open(&db_path).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::MapperDrugsBatch,
            queue: "map".to_string(),
            idempotency_key: "map-item-failure-v1".to_string(),
            input: json!({}),
            total: 1,
        })
        .unwrap();

    store
        .record_item_failure_with_input(
            &job.id,
            "src_bad",
            json!({"source_name": "missing query"}),
            json!({"code": "EMBEDDING_FAILED"}),
        )
        .unwrap();

    let conn = rusqlite::Connection::open(db_path).unwrap();
    let (state, input_json, error_json): (String, String, String) = conn
        .query_row(
            "SELECT state, input_json, error_json FROM job_items WHERE job_id = ?1 AND item_key = ?2",
            (&job.id, "src_bad"),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let input: serde_json::Value = serde_json::from_str(&input_json).unwrap();
    let error: serde_json::Value = serde_json::from_str(&error_json).unwrap();

    assert_eq!(state, "failed");
    assert_eq!(input["source_name"], "missing query");
    assert_eq!(error["code"], "EMBEDDING_FAILED");
}

#[test]
fn finish_success_stores_result_json() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::CatalogBuild,
            queue: "catalog".to_string(),
            idempotency_key: "catalog-v1".to_string(),
            input: json!({"athena_dir": "/fixtures/athena-mini"}),
            total: 0,
        })
        .unwrap();

    store.claim_next(&["catalog"]).unwrap();
    let done = store
        .finish_success(&job.id, json!({"catalog_artifact_id": "catalog-v1"}))
        .unwrap();
    let result = store.result_json(&job.id).unwrap().unwrap();

    assert_eq!(done.state, JobState::Succeeded);
    assert_eq!(result["catalog_artifact_id"], "catalog-v1");
    assert!(done.finished_at.is_some());
}

#[test]
fn finish_success_with_artifact_stores_artifact_path() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::MapperDrugsBatch,
            queue: "map".to_string(),
            idempotency_key: "map-artifact-v1".to_string(),
            input: json!({}),
            total: 2,
        })
        .unwrap();

    let artifact_path = dir.path().join("results/job/results.jsonl");
    let done = store
        .finish_success_with_artifact(
            &job.id,
            json!({"result_artifact_id": "job_results_v1"}),
            &artifact_path,
        )
        .unwrap();

    assert_eq!(done.state, JobState::Succeeded);
    assert_eq!(
        done.artifact_path.as_deref(),
        Some(artifact_path.to_str().unwrap())
    );
    assert_eq!(
        store.artifact_path(&job.id).unwrap().as_deref(),
        Some(artifact_path.to_str().unwrap())
    );
}

#[test]
fn finish_itemized_with_artifact_marks_succeeded_with_errors_and_stores_path() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::MapperDrugsBatch,
            queue: "map".to_string(),
            idempotency_key: "map-itemized-artifact-v1".to_string(),
            input: json!({}),
            total: 2,
        })
        .unwrap();
    store
        .record_item_failure(&job.id, "bad", json!({"code": "EMBEDDING_FAILED"}))
        .unwrap();

    let artifact_path = dir.path().join("results/job/results.jsonl");
    let done = store
        .finish_itemized_with_artifact(
            &job.id,
            2,
            json!({"result_artifact_id": "job_results_v1", "processed": 2, "failed": 1}),
            &artifact_path,
        )
        .unwrap();

    assert_eq!(done.state, JobState::SucceededWithErrors);
    assert_eq!(done.processed, 2);
    assert_eq!(done.failed, 1);
    assert_eq!(
        done.artifact_path.as_deref(),
        Some(artifact_path.to_str().unwrap())
    );
}

#[test]
fn record_event_assigns_monotonic_sequence_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::CatalogBuild,
            queue: "catalog".to_string(),
            idempotency_key: "catalog-events-v1".to_string(),
            input: json!({}),
            total: 0,
        })
        .unwrap();

    store
        .record_event(&job.id, "info", "started", Some(json!({"stage": "build"})))
        .unwrap();
    store
        .record_event(&job.id, "info", "finished", None)
        .unwrap();
    let events = store.events(&job.id).unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].seq, 1);
    assert_eq!(events[0].payload.as_ref().unwrap()["stage"], "build");
    assert_eq!(events[1].seq, 2);
}

#[test]
fn set_stage_updates_job_and_records_event() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::MapperDrugsBatch,
            queue: "map".to_string(),
            idempotency_key: "map-stage-v1".to_string(),
            input: json!({}),
            total: 2,
        })
        .unwrap();

    let staged = store
        .set_stage(
            &job.id,
            "bimaxsim_reranking",
            Some(json!({"processed": 1, "total": 2})),
        )
        .unwrap();
    let events = store.events(&job.id).unwrap();

    assert_eq!(staged.stage.as_deref(), Some("bimaxsim_reranking"));
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().stage.as_deref(),
        Some("bimaxsim_reranking")
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].message, "stage:bimaxsim_reranking");
    assert_eq!(
        events[0].payload.as_ref().unwrap()["stage"],
        "bimaxsim_reranking"
    );
    assert_eq!(events[0].payload.as_ref().unwrap()["processed"], 1);
}

#[test]
fn set_progress_updates_counts_and_records_event() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::SapbertBuild,
            queue: "embed".to_string(),
            idempotency_key: "sapbert-progress-v1".to_string(),
            input: json!({}),
            total: 0,
        })
        .unwrap();

    let updated = store
        .set_progress(
            &job.id,
            1_000,
            3_528_860,
            Some(json!({"stage": "embedding_documents"})),
        )
        .unwrap();
    let events = store.events(&job.id).unwrap();

    assert_eq!(updated.processed, 1_000);
    assert_eq!(updated.total, 3_528_860);
    assert_eq!(store.get(&job.id).unwrap().unwrap().processed, 1_000);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].message, "progress");
    assert_eq!(events[0].payload.as_ref().unwrap()["processed"], 1_000);
    assert_eq!(events[0].payload.as_ref().unwrap()["total"], 3_528_860);
    assert_eq!(
        events[0].payload.as_ref().unwrap()["stage"],
        "embedding_documents"
    );
}

#[test]
fn cancel_and_retry_follow_documented_state_transitions() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::MapperDrugsBatch,
            queue: "map".to_string(),
            idempotency_key: "map-cancel-v1".to_string(),
            input: json!({}),
            total: 2,
        })
        .unwrap();

    let cancelled = store.cancel(&job.id).unwrap();
    assert_eq!(cancelled.state, JobState::Cancelled);
    assert!(cancelled.finished_at.is_some());

    let retried = store.retry(&job.id).unwrap();
    assert_eq!(retried.state, JobState::Queued);
    assert_ne!(retried.id, job.id);
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().state,
        JobState::Cancelled
    );
    assert_eq!(retried.processed, 0);
    assert_eq!(retried.failed, 0);
    assert!(retried.started_at.is_none());
    assert!(retried.finished_at.is_none());
}

#[test]
fn finish_failed_stores_error_json_for_job_results_endpoint() {
    let dir = tempfile::tempdir().unwrap();
    let store = JobStore::open(dir.path().join("jobs.sqlite")).unwrap();
    store.migrate().unwrap();

    let job = store
        .create_job(CreateJob {
            kind: JobKind::SapbertBuild,
            queue: "embed".to_string(),
            idempotency_key: "sapbert-fail-v1".to_string(),
            input: json!({}),
            total: 0,
        })
        .unwrap();
    store
        .finish_failed(
            &job.id,
            json!({"code": "MODEL_NOT_READY", "message": "missing model"}),
        )
        .unwrap();

    let error = store.error_json(&job.id).unwrap().unwrap();
    assert_eq!(error["code"], "MODEL_NOT_READY");
}
