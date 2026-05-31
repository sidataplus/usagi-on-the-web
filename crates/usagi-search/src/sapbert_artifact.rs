use std::path::PathBuf;

use usagi_common::error::{ErrorCode, Result, UsagiError};

#[derive(Debug, Clone)]
pub struct SapbertArtifactPaths {
    pub artifact_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SapbertArtifact {
    pub artifact_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub usearch_path: PathBuf,
    pub concept_ids_path: PathBuf,
}

pub fn validate_sapbert_artifact(paths: SapbertArtifactPaths) -> Result<SapbertArtifact> {
    let manifest_path = paths.artifact_dir.join("manifest.json");
    let usearch_path = paths.artifact_dir.join("sapbert_cls.usearch");
    let concept_ids_path = paths.artifact_dir.join("concept_ids.arrow");

    for path in [&manifest_path, &usearch_path, &concept_ids_path] {
        if !path.exists() {
            return Err(UsagiError::new(
                ErrorCode::IndexNotReady,
                format!("SapBERT artifact is missing {}", path.display()),
            ));
        }
    }

    Ok(SapbertArtifact {
        artifact_dir: paths.artifact_dir,
        manifest_path,
        usearch_path,
        concept_ids_path,
    })
}
