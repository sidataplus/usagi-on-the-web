use serde_json::json;
use usagi_artifacts::job_results::{
    job_results_response, read_job_result_artifact, write_mapper_batch_results,
    JobResultArtifactOptions,
};
use usagi_common::manifest::{sha256_file, ArtifactManifest};

#[test]
fn writes_mapper_batch_results_jsonl_and_manifest_atomically() {
    let dir = tempfile::tempdir().unwrap();

    let artifact = write_mapper_batch_results(
        JobResultArtifactOptions {
            results_root: dir.path().join("results"),
            job_id: "job_map_abc".to_string(),
            artifact_id: None,
            provenance: Some(json!({
                "model_artifact_id": "sidataplus/THIRAWAT-SapBERT",
                "index_artifact_id": "tachiom-v1"
            })),
        },
        [
            json!({"id": "src_001", "candidates": []}),
            json!({"id": "src_002", "error": {"code": "MODEL_NOT_READY"}}),
        ],
    )
    .unwrap();

    let results_text = std::fs::read_to_string(&artifact.path).unwrap();
    assert_eq!(results_text.lines().count(), 2);
    assert_eq!(artifact.content_type, "application/jsonl");
    assert_eq!(artifact.sha256, sha256_file(&artifact.path).unwrap());

    let manifest: ArtifactManifest =
        serde_json::from_slice(&std::fs::read(&artifact.manifest_path).unwrap()).unwrap();
    let manifest_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&artifact.manifest_path).unwrap()).unwrap();
    assert_eq!(manifest.artifact_id, "job_map_abc_results_v1");
    assert_eq!(manifest_json["job_id"], "job_map_abc");
    assert_eq!(manifest.artifact_kind, "mapper-batch-results");
    assert_eq!(
        manifest.extra.as_ref().unwrap()["provenance"]["model_artifact_id"],
        "sidataplus/THIRAWAT-SapBERT"
    );
    assert_eq!(manifest.outputs[0].path, "results.jsonl");
    assert_eq!(
        manifest.outputs[0].content_type.as_deref(),
        Some("application/jsonl")
    );
    assert_eq!(
        manifest.outputs[0].sha256.as_deref(),
        Some(artifact.sha256.as_str())
    );

    let read_back = read_job_result_artifact(&artifact.path).unwrap();
    assert_eq!(read_back, artifact);

    let response = job_results_response(
        "job_map_abc",
        "succeeded",
        Some(json!({"ignored": true})),
        None,
        Some(artifact.path.clone()),
    )
    .unwrap();
    assert_eq!(
        response["artifact"]["artifact_id"],
        "job_map_abc_results_v1"
    );
    assert_eq!(
        response["artifact"]["provenance"]["index_artifact_id"],
        "tachiom-v1"
    );
    assert!(response.get("result").is_none());
}

#[test]
fn refuses_to_overwrite_existing_job_result_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let options = JobResultArtifactOptions {
        results_root: dir.path().join("results"),
        job_id: "job_map_abc".to_string(),
        artifact_id: None,
        provenance: None,
    };

    write_mapper_batch_results(options.clone(), [json!({"id": "src_001"})]).unwrap();
    let err = write_mapper_batch_results(options, [json!({"id": "src_002"})]).unwrap_err();

    assert_eq!(err.code().as_str(), "INCOMPATIBLE_ARTIFACT");
}
