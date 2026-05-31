use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_common::manifest::{sha256_file, ArtifactManifest, ManifestFile};
use usagi_thirawat::artifact::{
    validate_tachiom_artifact, validate_thirawat_doc_embedding_artifact, TachiomArtifactPaths,
    ThirawatDocEmbeddingArtifactPaths,
};
use usagi_thirawat::mapper::{TachiomFixtureDocument, TachiomFixtureIndex};

#[derive(Debug, Clone)]
pub struct TachiomBuildOptions {
    pub doc_embedding_dir: PathBuf,
    pub index_dir: PathBuf,
    pub artifact_id: Option<String>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TachiomBuildSummary {
    pub tachiom_artifact_id: String,
    pub document_count: usize,
    pub index_path: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocEmbeddingFixture {
    pub documents: Vec<TachiomFixtureDocument>,
}

pub fn build_tachiom_index(options: TachiomBuildOptions) -> Result<TachiomBuildSummary> {
    validate_thirawat_doc_embedding_artifact(ThirawatDocEmbeddingArtifactPaths {
        doc_embedding_dir: options.doc_embedding_dir.clone(),
    })?;
    if options.index_dir.exists() && !options.overwrite {
        return Err(UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "Tachiom index already exists at {}",
                options.index_dir.display()
            ),
        ));
    }
    if options.index_dir.exists() {
        std::fs::remove_dir_all(&options.index_dir)?;
    }
    std::fs::create_dir_all(&options.index_dir)?;

    let fixture_path = options.doc_embedding_dir.join("documents.json");
    let fixture: DocEmbeddingFixture = serde_json::from_slice(&std::fs::read(&fixture_path)?)
        .map_err(|err| {
            UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                format!(
                    "THIRAWAT fixture document embeddings {} are not valid JSON: {err}",
                    fixture_path.display()
                ),
            )
        })?;
    validate_fixture_documents(&fixture.documents)?;

    let index = TachiomFixtureIndex {
        documents: fixture.documents,
    };
    let index_path = options.index_dir.join("index.bin");
    std::fs::write(&index_path, serde_json::to_vec_pretty(&index)?)?;
    let index_sha256 = sha256_file(&index_path)?;
    let doc_manifest_sha256 = sha256_file(options.doc_embedding_dir.join("manifest.json"))?;
    let artifact_id = options
        .artifact_id
        .unwrap_or_else(|| "local-thirawat-drug-tachiom-v1".to_string());
    let manifest = ArtifactManifest {
        artifact_id: artifact_id.clone(),
        artifact_kind: "tachiom-index".to_string(),
        schema_version: "usagi-tachiom-v1".to_string(),
        api_version: "0.1.0".to_string(),
        created_at: Utc::now().to_rfc3339(),
        inputs: vec![ManifestFile {
            path: "doc_embeddings/manifest.json".to_string(),
            sha256: Some(doc_manifest_sha256),
            content_type: Some("application/json".to_string()),
        }],
        outputs: vec![ManifestFile {
            path: "index.bin".to_string(),
            sha256: Some(index_sha256),
            content_type: Some("application/octet-stream".to_string()),
        }],
        extra: Some(serde_json::json!({
            "retrieval": {
                "engine": "tachiom-fixture",
                "metric": "maxsim"
            },
            "build_params": {
                "token_aware_clustering": false
            }
        })),
    };
    let manifest_path = options.index_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    validate_tachiom_artifact(TachiomArtifactPaths {
        index_dir: options.index_dir.clone(),
    })?;

    Ok(TachiomBuildSummary {
        tachiom_artifact_id: artifact_id,
        document_count: index.documents.len(),
        index_path: index_path.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
    })
}

fn validate_fixture_documents(documents: &[TachiomFixtureDocument]) -> Result<()> {
    if documents.is_empty() {
        return Err(UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            "THIRAWAT fixture document embeddings must contain at least one document",
        ));
    }
    for document in documents {
        if document.concept.domain_id != "Drug" || document.concept.standard_concept != "S" {
            return Err(UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                "Tachiom Drug index can only include standard Drug concepts",
            ));
        }
        if document.token_vectors.is_empty() {
            return Err(UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                format!(
                    "document {} has no token vectors",
                    document.concept.concept_id
                ),
            ));
        }
    }
    Ok(())
}
