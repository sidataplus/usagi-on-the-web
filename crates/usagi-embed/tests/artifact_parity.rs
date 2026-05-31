use std::fs;

use usagi_common::error::ErrorCode;
use usagi_embed::artifact::{validate_sapbert_model_artifact, SapbertModelArtifactPaths};
use usagi_embed::parity::{
    assert_cls_parity, assert_token_parity, cosine_similarity, l2_normalize,
};
use usagi_embed::types::{ClsEmbedding, TokenEmbedding};

#[test]
fn sapbert_model_artifact_requires_documented_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config.json"), "{}").unwrap();
    fs::write(dir.path().join("tokenizer.json"), "{}").unwrap();

    let err = validate_sapbert_model_artifact(SapbertModelArtifactPaths {
        model_dir: dir.path().to_path_buf(),
    })
    .unwrap_err();

    assert_eq!(err.code(), ErrorCode::ModelNotReady);
    assert!(err.message().contains("model.safetensors"));
}

#[test]
fn sapbert_model_artifact_accepts_complete_shape() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config.json"), "{}").unwrap();
    fs::write(dir.path().join("tokenizer.json"), "{}").unwrap();
    fs::write(dir.path().join("model.safetensors"), "weights").unwrap();
    fs::write(dir.path().join("manifest.json"), "{}").unwrap();

    let artifact = validate_sapbert_model_artifact(SapbertModelArtifactPaths {
        model_dir: dir.path().to_path_buf(),
    })
    .unwrap();

    assert_eq!(artifact.tokenizer_json, dir.path().join("tokenizer.json"));
    assert_eq!(
        artifact.model_safetensors,
        dir.path().join("model.safetensors")
    );
}

#[test]
fn l2_normalize_produces_unit_vector() {
    let normalized = l2_normalize(&[3.0, 4.0]).unwrap();
    assert!((normalized[0] - 0.6).abs() < 1e-6);
    assert!((normalized[1] - 0.8).abs() < 1e-6);
    assert!((cosine_similarity(&normalized, &normalized).unwrap() - 1.0).abs() < 1e-6);
}

#[test]
fn cls_parity_requires_cosine_threshold() {
    let candidate = ClsEmbedding {
        text: "tramadol".to_string(),
        vector: l2_normalize(&[1.0, 0.0]).unwrap(),
    };
    let reference = ClsEmbedding {
        text: "tramadol".to_string(),
        vector: l2_normalize(&[0.0, 1.0]).unwrap(),
    };

    let err = assert_cls_parity(&candidate, &reference, 0.999).unwrap_err();
    assert_eq!(err.code(), ErrorCode::EmbeddingFailed);
}

#[test]
fn cls_parity_accepts_matching_vectors() {
    let candidate = ClsEmbedding {
        text: "tramadol".to_string(),
        vector: l2_normalize(&[1.0, 2.0, 3.0]).unwrap(),
    };
    let reference = ClsEmbedding {
        text: "tramadol".to_string(),
        vector: l2_normalize(&[1.0, 2.0, 3.0]).unwrap(),
    };

    assert_cls_parity(&candidate, &reference, 0.999).unwrap();
}

#[test]
fn token_parity_requires_exact_token_ids_and_attention_mask() {
    let candidate = TokenEmbedding {
        text: "tramadol".to_string(),
        token_ids: vec![0, 10, 20],
        vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        attention_mask: vec![true, true, false],
    };
    let reference = TokenEmbedding {
        text: "tramadol".to_string(),
        token_ids: vec![0, 10, 21],
        vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        attention_mask: vec![true, true, true],
    };

    let err = assert_token_parity(&candidate, &reference, 0.999, 96, 2).unwrap_err();

    assert_eq!(err.code(), ErrorCode::EmbeddingFailed);
    assert!(err.message().contains("token_ids"));
}

#[test]
fn token_parity_requires_mean_token_cosine_threshold() {
    let candidate = TokenEmbedding {
        text: "tramadol".to_string(),
        token_ids: vec![0, 10],
        vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        attention_mask: vec![true, true],
    };
    let reference = TokenEmbedding {
        text: "tramadol".to_string(),
        token_ids: vec![0, 10],
        vectors: vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        attention_mask: vec![true, true],
    };

    let err = assert_token_parity(&candidate, &reference, 0.999, 96, 2).unwrap_err();

    assert_eq!(err.code(), ErrorCode::EmbeddingFailed);
    assert!(err.message().contains("mean token cosine"));
}

#[test]
fn token_parity_accepts_matching_token_embeddings() {
    let candidate = TokenEmbedding {
        text: "tramadol".to_string(),
        token_ids: vec![0, 10],
        vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        attention_mask: vec![true, true],
    };
    let reference = candidate.clone();

    assert_token_parity(&candidate, &reference, 0.999, 96, 2).unwrap();
}
