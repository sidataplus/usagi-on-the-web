use std::path::PathBuf;

use usagi_common::error::{ErrorCode, Result, UsagiError};

#[derive(Debug, Clone)]
pub struct SapbertModelArtifactPaths {
    pub model_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SapbertModelArtifact {
    pub model_dir: PathBuf,
    pub config_json: PathBuf,
    pub tokenizer_json: PathBuf,
    pub model_safetensors: PathBuf,
    pub manifest_json: PathBuf,
}

pub fn validate_sapbert_model_artifact(
    paths: SapbertModelArtifactPaths,
) -> Result<SapbertModelArtifact> {
    let artifact = SapbertModelArtifact {
        config_json: paths.model_dir.join("config.json"),
        tokenizer_json: paths.model_dir.join("tokenizer.json"),
        model_safetensors: paths.model_dir.join("model.safetensors"),
        manifest_json: paths.model_dir.join("manifest.json"),
        model_dir: paths.model_dir,
    };

    for path in [
        &artifact.config_json,
        &artifact.tokenizer_json,
        &artifact.model_safetensors,
        &artifact.manifest_json,
    ] {
        if !path.exists() {
            return Err(UsagiError::new(
                ErrorCode::ModelNotReady,
                format!("SapBERT model artifact is missing {}", path.display()),
            ));
        }
    }

    Ok(artifact)
}
