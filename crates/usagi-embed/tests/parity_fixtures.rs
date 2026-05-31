use serde_json::json;
use usagi_common::error::ErrorCode;
use usagi_embed::parity::{assert_cls_parity_files, assert_token_parity_files};

#[test]
fn cls_parity_can_be_checked_from_json_files() {
    let dir = tempfile::tempdir().unwrap();
    let candidate = dir.path().join("candidate-cls.json");
    let reference = dir.path().join("reference-cls.json");
    std::fs::write(
        &candidate,
        serde_json::to_vec_pretty(&json!({
            "text": "tramadol",
            "vector": [0.6, 0.8]
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        &reference,
        serde_json::to_vec_pretty(&json!({
            "text": "tramadol",
            "vector": [0.6, 0.8]
        }))
        .unwrap(),
    )
    .unwrap();

    assert_cls_parity_files(&candidate, &reference, 0.999).unwrap();
}

#[test]
fn token_parity_can_be_checked_from_json_files() {
    let dir = tempfile::tempdir().unwrap();
    let candidate = dir.path().join("candidate-token.json");
    let reference = dir.path().join("reference-token.json");
    let fixture = json!({
        "text": "tramadol",
        "token_ids": [0, 10],
        "vectors": [[1.0, 0.0], [0.0, 1.0]],
        "attention_mask": [true, true]
    });
    std::fs::write(&candidate, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();
    std::fs::write(&reference, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();

    assert_token_parity_files(&candidate, &reference, 0.999, 96, 2).unwrap();
}

#[test]
fn invalid_parity_fixture_json_is_an_incompatible_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let candidate = dir.path().join("candidate-cls.json");
    let reference = dir.path().join("reference-cls.json");
    std::fs::write(&candidate, b"{").unwrap();
    std::fs::write(
        &reference,
        serde_json::to_vec_pretty(&json!({
            "text": "tramadol",
            "vector": [0.6, 0.8]
        }))
        .unwrap(),
    )
    .unwrap();

    let err = assert_cls_parity_files(&candidate, &reference, 0.999).unwrap_err();

    assert_eq!(err.code(), ErrorCode::IncompatibleArtifact);
}
