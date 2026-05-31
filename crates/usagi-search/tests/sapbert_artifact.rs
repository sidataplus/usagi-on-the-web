use std::fs;

use usagi_common::error::ErrorCode;
use usagi_search::sapbert_artifact::{validate_sapbert_artifact, SapbertArtifactPaths};

#[test]
fn sapbert_artifact_validation_requires_documented_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("manifest.json"), "{}").unwrap();
    fs::write(dir.path().join("sapbert_cls.usearch"), "index").unwrap();

    let err = validate_sapbert_artifact(SapbertArtifactPaths {
        artifact_dir: dir.path().to_path_buf(),
    })
    .unwrap_err();

    assert_eq!(err.code(), ErrorCode::IndexNotReady);
    assert!(err.message().contains("concept_ids.arrow"));
}

#[test]
fn sapbert_artifact_validation_accepts_complete_shape() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("manifest.json"), "{}").unwrap();
    fs::write(dir.path().join("sapbert_cls.usearch"), "index").unwrap();
    fs::write(dir.path().join("concept_ids.arrow"), "ids").unwrap();

    let artifact = validate_sapbert_artifact(SapbertArtifactPaths {
        artifact_dir: dir.path().to_path_buf(),
    })
    .unwrap();

    assert_eq!(
        artifact.usearch_path,
        dir.path().join("sapbert_cls.usearch")
    );
    assert_eq!(
        artifact.concept_ids_path,
        dir.path().join("concept_ids.arrow")
    );
}
