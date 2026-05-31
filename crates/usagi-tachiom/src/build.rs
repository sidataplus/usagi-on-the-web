use std::path::PathBuf;
use std::process::Command;

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
    pub backend: TachiomBuildBackend,
}

#[derive(Debug, Clone)]
pub enum TachiomBuildBackend {
    FixtureExact,
    Cli(TachiomCliBuildOptions),
}

#[derive(Debug, Clone)]
pub struct TachiomCliBuildOptions {
    pub build_bin: PathBuf,
    pub total_centroids: Option<usize>,
    pub normalize: bool,
}

pub fn tachiom_backend_from_env() -> Result<TachiomBuildBackend> {
    match std::env::var("TACHIOM_BACKEND")
        .unwrap_or_else(|_| "cli".to_string())
        .as_str()
    {
        "fixture" | "fixture-exact" => Ok(TachiomBuildBackend::FixtureExact),
        "cli" => {
            let build_bin = std::env::var("TACHIOM_BUILD_BIN")
                .map(PathBuf::from)
                .map_err(|_| {
                    UsagiError::new(
                        ErrorCode::TachiomFailed,
                        "TACHIOM_BUILD_BIN is required when TACHIOM_BACKEND=cli",
                    )
                })?;
            let total_centroids = std::env::var("TACHIOM_TOTAL_CENTROIDS")
                .ok()
                .map(|value| {
                    value.parse::<usize>().map_err(|err| {
                        UsagiError::bad_request(format!(
                            "TACHIOM_TOTAL_CENTROIDS must be an integer: {err}"
                        ))
                    })
                })
                .transpose()?;
            let normalize = std::env::var("TACHIOM_NORMALIZE")
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(true);
            Ok(TachiomBuildBackend::Cli(TachiomCliBuildOptions {
                build_bin,
                total_centroids,
                normalize,
            }))
        }
        other => Err(UsagiError::bad_request(format!(
            "unsupported TACHIOM_BACKEND {other:?}; expected cli or fixture"
        ))),
    }
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

    let index_path = options.index_dir.join("index.bin");
    let (document_count, backend_extra) = match &options.backend {
        TachiomBuildBackend::FixtureExact => {
            let fixture_path = options.doc_embedding_dir.join("documents.json");
            let fixture: DocEmbeddingFixture =
                serde_json::from_slice(&std::fs::read(&fixture_path)?).map_err(|err| {
                    UsagiError::new(
                        ErrorCode::IncompatibleArtifact,
                        format!(
                            "THIRAWAT fixture document embeddings {} are not valid JSON: {err}",
                            fixture_path.display()
                        ),
                    )
                })?;
            validate_fixture_documents(&fixture.documents)?;
            let document_count = fixture.documents.len();
            let index = TachiomFixtureIndex {
                documents: fixture.documents,
            };
            std::fs::write(&index_path, serde_json::to_vec_pretty(&index)?)?;
            (
                document_count,
                serde_json::json!({
                    "retrieval": {
                        "engine": "fixture-exact-maxsim",
                        "metric": "maxsim",
                        "input_format": "documents-json"
                    },
                    "build_params": {
                        "token_aware_clustering": false
                    }
                }),
            )
        }
        TachiomBuildBackend::Cli(cli) => {
            run_tachiom_build_cli(cli, &options.doc_embedding_dir, &index_path)?;
            (
                document_count_from_doc_artifact(&options.doc_embedding_dir)?,
                serde_json::json!({
                    "retrieval": {
                        "engine": "tachiom-cli",
                        "metric": "maxsim",
                        "input_format": "tachiom-npy"
                    },
                    "build_params": {
                        "total_centroids": cli.total_centroids,
                        "normalize": cli.normalize
                    }
                }),
            )
        }
    };
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
        extra: Some(backend_extra),
    };
    let manifest_path = options.index_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    validate_tachiom_artifact(TachiomArtifactPaths {
        index_dir: options.index_dir.clone(),
    })?;

    Ok(TachiomBuildSummary {
        tachiom_artifact_id: artifact_id,
        document_count,
        index_path: index_path.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
    })
}

fn run_tachiom_build_cli(
    cli: &TachiomCliBuildOptions,
    doc_embedding_dir: &std::path::Path,
    index_path: &std::path::Path,
) -> Result<()> {
    if !cli.build_bin.exists() {
        return Err(UsagiError::new(
            ErrorCode::TachiomFailed,
            format!(
                "Tachiom build binary is missing {}",
                cli.build_bin.display()
            ),
        ));
    }
    let mut command = Command::new(&cli.build_bin);
    command
        .arg("-i")
        .arg(doc_embedding_dir.join("token_vectors.npy"))
        .arg("--token-ids-file")
        .arg(doc_embedding_dir.join("token_ids.npy"))
        .arg("--doclens-file")
        .arg(doc_embedding_dir.join("doclens.npy"))
        .arg("-o")
        .arg(index_path);
    if let Some(total_centroids) = cli.total_centroids {
        command
            .arg("--total-centroids")
            .arg(total_centroids.to_string());
    }
    if cli.normalize {
        command.arg("--normalize");
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(UsagiError::new(
            ErrorCode::TachiomFailed,
            format!(
                "Tachiom build failed with status {}: {}{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    if !index_path.exists() {
        return Err(UsagiError::new(
            ErrorCode::TachiomFailed,
            format!(
                "Tachiom build did not create expected index {}",
                index_path.display()
            ),
        ));
    }
    Ok(())
}

fn document_count_from_doc_artifact(doc_embedding_dir: &std::path::Path) -> Result<usize> {
    let manifest_path = doc_embedding_dir.join("manifest.json");
    let manifest: ArtifactManifest = serde_json::from_slice(&std::fs::read(&manifest_path)?)?;
    manifest
        .extra
        .and_then(|extra| {
            extra
                .get("counts")
                .and_then(|counts| counts.get("documents"))
                .and_then(|documents| documents.as_u64())
        })
        .map(|documents| documents as usize)
        .ok_or_else(|| {
            UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                "THIRAWAT document embedding manifest is missing counts.documents",
            )
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
