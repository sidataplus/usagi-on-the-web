use serde_json::json;
use usagi_common::manifest::sha256_file;
use usagi_thirawat::artifact::{validate_thirawat_model_artifact, ThirawatModelArtifactPaths};

#[test]
fn thirawat_model_manifest_must_declare_merged_peft_weights() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_model_files(dir.path(), false, None);

    let err = validate_thirawat_model_artifact(ThirawatModelArtifactPaths {
        model_dir: dir.path().to_path_buf(),
    })
    .expect_err("unmerged PEFT artifact must be rejected");

    assert!(err.message().contains("peft.merged"));
}

#[test]
fn thirawat_model_manifest_checksums_must_match_weights() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_model_files(dir.path(), true, Some(BadChecksum::Model));

    let err = validate_thirawat_model_artifact(ThirawatModelArtifactPaths {
        model_dir: dir.path().to_path_buf(),
    })
    .expect_err("checksum mismatch must be rejected");

    assert!(err.message().contains("checksum"));
}

#[test]
fn thirawat_model_manifest_accepts_documented_runtime_shape() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_model_files(dir.path(), true, None);

    validate_thirawat_model_artifact(ThirawatModelArtifactPaths {
        model_dir: dir.path().to_path_buf(),
    })
    .expect("documented THIRAWAT artifact shape should validate");
}

#[derive(Debug, Clone, Copy)]
enum BadChecksum {
    Model,
}

fn write_model_files(dir: &std::path::Path, merged: bool, bad_checksum: Option<BadChecksum>) {
    std::fs::write(dir.join("config.json"), "{}").expect("config");
    std::fs::write(dir.join("tokenizer.json"), "{}").expect("tokenizer");
    std::fs::write(dir.join("tokenizer_config.json"), "{}").expect("tokenizer config");
    std::fs::write(dir.join("special_tokens_map.json"), "{}").expect("special tokens");
    std::fs::write(dir.join("model.safetensors"), "merged-model-weights").expect("model");
    std::fs::write(dir.join("colbert_projection.safetensors"), "projection").expect("projection");

    let mut model_sha = sha256_file(dir.join("model.safetensors")).unwrap();
    if matches!(bad_checksum, Some(BadChecksum::Model)) {
        model_sha = "definitely-not-the-model-sha".to_string();
    }
    let checksums = json!({
        "model.safetensors": model_sha,
        "colbert_projection.safetensors": sha256_file(dir.join("colbert_projection.safetensors")).unwrap()
    });
    let manifest = json!({
        "model_id": "sidataplus/THIRAWAT-SapBERT",
        "base_model": "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR",
        "architecture": "pylate_colbert",
        "encoder_family": "xlm-roberta",
        "query_length": 96,
        "document_length": 96,
        "hidden_dim": 768,
        "projection_dim": 128,
        "similarity": "maxsim",
        "projection": {
            "in_features": 768,
            "out_features": 128,
            "bias": false
        },
        "peft": {
            "merged": merged
        },
        "sha256": checksums
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("manifest json"),
    )
    .expect("manifest");
}
