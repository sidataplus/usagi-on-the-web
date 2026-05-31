use std::path::PathBuf;

use serde::Deserialize;
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_common::manifest::verify_file_sha256;

#[derive(Debug, Clone)]
pub struct ThirawatModelArtifactPaths {
    pub model_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ThirawatDocEmbeddingArtifactPaths {
    pub doc_embedding_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TachiomArtifactPaths {
    pub index_dir: PathBuf,
}

pub fn validate_thirawat_model_artifact(paths: ThirawatModelArtifactPaths) -> Result<()> {
    require_files(
        &paths.model_dir,
        &[
            "config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "special_tokens_map.json",
            "model.safetensors",
            "colbert_projection.safetensors",
            "manifest.json",
        ],
        ErrorCode::ModelNotReady,
        "THIRAWAT-SapBERT model artifact",
    )?;
    validate_thirawat_model_manifest(&paths.model_dir)
}

pub fn validate_thirawat_doc_embedding_artifact(
    paths: ThirawatDocEmbeddingArtifactPaths,
) -> Result<()> {
    require_files(
        &paths.doc_embedding_dir,
        &[
            "token_vectors.npy",
            "token_ids.npy",
            "doclens.npy",
            "doc_ids.arrow",
            "manifest.json",
        ],
        ErrorCode::IndexNotReady,
        "THIRAWAT Drug document embedding artifact",
    )
}

pub fn validate_tachiom_artifact(paths: TachiomArtifactPaths) -> Result<()> {
    require_files(
        &paths.index_dir,
        &["index.bin", "manifest.json"],
        ErrorCode::IndexNotReady,
        "Tachiom index artifact",
    )
}

fn require_files(
    dir: &std::path::Path,
    filenames: &[&str],
    error_code: ErrorCode,
    label: &str,
) -> Result<()> {
    for filename in filenames {
        let path = dir.join(filename);
        if !path.exists() {
            return Err(UsagiError::new(
                error_code,
                format!("{label} is missing {}", path.display()),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ThirawatModelManifest {
    model_id: String,
    base_model: String,
    architecture: String,
    encoder_family: String,
    query_length: usize,
    document_length: usize,
    hidden_dim: usize,
    projection_dim: usize,
    similarity: String,
    projection: ProjectionManifest,
    peft: PeftManifest,
    sha256: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ProjectionManifest {
    in_features: usize,
    out_features: usize,
    bias: bool,
}

#[derive(Debug, Deserialize)]
struct PeftManifest {
    merged: bool,
}

fn validate_thirawat_model_manifest(model_dir: &std::path::Path) -> Result<()> {
    let manifest_path = model_dir.join("manifest.json");
    let manifest: ThirawatModelManifest = serde_json::from_slice(&std::fs::read(&manifest_path)?)
        .map_err(|err| {
        UsagiError::incompatible_artifact(format!(
            "THIRAWAT-SapBERT model manifest {} is invalid: {err}",
            manifest_path.display()
        ))
    })?;

    require_manifest_value(
        manifest.model_id == "sidataplus/THIRAWAT-SapBERT",
        "model_id must be sidataplus/THIRAWAT-SapBERT",
    )?;
    require_manifest_value(
        manifest.base_model == "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR",
        "base_model must be cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR",
    )?;
    require_manifest_value(
        manifest.architecture == "pylate_colbert",
        "architecture must be pylate_colbert",
    )?;
    require_manifest_value(
        manifest.encoder_family == "xlm-roberta",
        "encoder_family must be xlm-roberta",
    )?;
    require_manifest_value(manifest.query_length == 96, "query_length must be 96")?;
    require_manifest_value(manifest.document_length == 96, "document_length must be 96")?;
    require_manifest_value(manifest.hidden_dim == 768, "hidden_dim must be 768")?;
    require_manifest_value(manifest.projection_dim == 128, "projection_dim must be 128")?;
    require_manifest_value(manifest.similarity == "maxsim", "similarity must be maxsim")?;
    require_manifest_value(
        manifest.projection.in_features == 768,
        "projection.in_features must be 768",
    )?;
    require_manifest_value(
        manifest.projection.out_features == 128,
        "projection.out_features must be 128",
    )?;
    require_manifest_value(!manifest.projection.bias, "projection.bias must be false")?;
    require_manifest_value(manifest.peft.merged, "peft.merged must be true")?;

    verify_manifest_sha(model_dir, &manifest.sha256, "model.safetensors")?;
    verify_manifest_sha(
        model_dir,
        &manifest.sha256,
        "colbert_projection.safetensors",
    )?;
    Ok(())
}

fn require_manifest_value(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(UsagiError::incompatible_artifact(format!(
            "THIRAWAT-SapBERT model manifest {message}"
        )))
    }
}

fn verify_manifest_sha(
    model_dir: &std::path::Path,
    checksums: &std::collections::BTreeMap<String, String>,
    filename: &str,
) -> Result<()> {
    let expected = checksums.get(filename).ok_or_else(|| {
        UsagiError::incompatible_artifact(format!(
            "THIRAWAT-SapBERT model manifest sha256 is missing {filename}"
        ))
    })?;
    verify_file_sha256(model_dir.join(filename), expected)
}
