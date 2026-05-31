use std::fs;

use usagi_common::error::{ErrorCode, ErrorEnvelope, UsagiError};
use usagi_common::manifest::{sha256_file, verify_file_sha256};

#[test]
fn error_envelope_serializes_with_request_id() {
    let err = UsagiError::bad_request("q is required").with_request_id("req_test");
    let envelope = ErrorEnvelope::from(err);
    let json = serde_json::to_value(&envelope).unwrap();

    assert_eq!(json["error"]["code"], "BAD_REQUEST");
    assert_eq!(json["error"]["message"], "q is required");
    assert_eq!(json["error"]["request_id"], "req_test");
}

#[test]
fn error_envelope_generates_request_id_when_missing() {
    let envelope = ErrorEnvelope::from(UsagiError::bad_request("q is required"));
    assert!(envelope.error.request_id.starts_with("req_"));
}

#[test]
fn error_code_has_documented_wire_names() {
    assert_eq!(ErrorCode::Unauthorized.as_str(), "UNAUTHORIZED");
    assert_eq!(ErrorCode::IndexNotReady.as_str(), "INDEX_NOT_READY");
    assert_eq!(
        ErrorCode::IncompatibleArtifact.as_str(),
        "INCOMPATIBLE_ARTIFACT"
    );
}

#[test]
fn sha256_verification_detects_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("artifact.txt");
    fs::write(&path, "catalog").unwrap();

    let digest = sha256_file(&path).unwrap();
    verify_file_sha256(&path, &digest).unwrap();

    fs::write(&path, "catalog-corrupted").unwrap();
    let err = verify_file_sha256(&path, &digest).unwrap_err();
    assert_eq!(err.code(), ErrorCode::IncompatibleArtifact);
}
