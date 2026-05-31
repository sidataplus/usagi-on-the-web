use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_common::manifest::{sha256_file, ArtifactManifest, ManifestFile};
use usagi_contracts::catalog::ConceptSummary;

use crate::artifact::{
    validate_thirawat_doc_embedding_artifact, ThirawatDocEmbeddingArtifactPaths,
};
use crate::mapper::{TachiomFixtureDocument, TachiomFixtureIndex};

#[derive(Debug, Clone)]
pub struct ThirawatDocEmbeddingBuildOptions {
    pub doc_embedding_dir: PathBuf,
    pub catalog_artifact_id: String,
    pub model_artifact_id: String,
    pub artifact_id: Option<String>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThirawatDocEmbeddingDocument {
    pub concept: ConceptSummary,
    pub token_ids: Vec<i64>,
    pub token_vectors: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThirawatDocEmbeddingBuildSummary {
    pub doc_embedding_artifact_id: String,
    pub document_count: usize,
    pub token_count: usize,
    pub dimension: usize,
    pub manifest_path: String,
}

pub fn write_thirawat_doc_embedding_artifact(
    options: ThirawatDocEmbeddingBuildOptions,
    documents: Vec<ThirawatDocEmbeddingDocument>,
) -> Result<ThirawatDocEmbeddingBuildSummary> {
    let (token_count, dimension) = validate_documents(&documents)?;
    let parent = options
        .doc_embedding_dir
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&parent)?;
    if options.doc_embedding_dir.exists() && !options.overwrite {
        return Err(UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "THIRAWAT document embedding artifact already exists at {}",
                options.doc_embedding_dir.display()
            ),
        ));
    }

    let temp_dir = parent.join(format!(
        ".{}.tmp-{}",
        options
            .doc_embedding_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("doc_embeddings"),
        std::process::id()
    ));
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }
    std::fs::create_dir_all(&temp_dir)?;

    let token_vectors = documents
        .iter()
        .flat_map(|document| document.token_vectors.iter())
        .flat_map(|vector| vector.iter().copied())
        .collect::<Vec<_>>();
    let token_ids = documents
        .iter()
        .flat_map(|document| document.token_ids.iter().copied())
        .collect::<Vec<_>>();
    let doclens = documents
        .iter()
        .map(|document| document.token_vectors.len() as i64)
        .collect::<Vec<_>>();
    let doc_ids = documents
        .iter()
        .map(|document| document.concept.concept_id)
        .collect::<Vec<_>>();
    write_npy_f32_2d(
        temp_dir.join("token_vectors.npy"),
        &token_vectors,
        token_count,
        dimension,
    )?;
    write_npy_i64_1d(temp_dir.join("token_ids.npy"), &token_ids)?;
    write_npy_i64_1d(temp_dir.join("doclens.npy"), &doclens)?;
    std::fs::write(
        temp_dir.join("doc_ids.arrow"),
        serde_json::to_vec(&serde_json::json!({ "concept_ids": doc_ids }))?,
    )?;
    std::fs::write(
        temp_dir.join("documents.json"),
        serde_json::to_vec_pretty(&TachiomFixtureIndex {
            documents: documents
                .iter()
                .map(|document| TachiomFixtureDocument {
                    concept: document.concept.clone(),
                    token_vectors: document.token_vectors.clone(),
                })
                .collect(),
        })?,
    )?;

    let artifact_id = options
        .artifact_id
        .unwrap_or_else(|| "local-thirawat-drug-doc-embeddings-v1".to_string());
    let manifest = ArtifactManifest {
        artifact_id: artifact_id.clone(),
        artifact_kind: "thirawat-doc-embeddings".to_string(),
        schema_version: "usagi-thirawat-doc-embeddings-v1".to_string(),
        api_version: "0.1.0".to_string(),
        created_at: Utc::now().to_rfc3339(),
        inputs: Vec::new(),
        outputs: manifest_outputs(&temp_dir)?,
        extra: Some(serde_json::json!({
            "catalog": {
                "artifact_id": options.catalog_artifact_id,
                "scope": {
                    "domain_id": "Drug",
                    "standard_concept": "S"
                }
            },
            "model": {
                "model_artifact_id": options.model_artifact_id,
                "architecture": "pylate_colbert",
                "embedding_kind": "projected_token_vectors"
            },
            "counts": {
                "documents": documents.len(),
                "tokens": token_count,
                "dimension": dimension
            }
        })),
    };
    let manifest_path = temp_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    validate_thirawat_doc_embedding_artifact(ThirawatDocEmbeddingArtifactPaths {
        doc_embedding_dir: temp_dir.clone(),
    })?;
    if options.doc_embedding_dir.exists() {
        std::fs::remove_dir_all(&options.doc_embedding_dir)?;
    }
    std::fs::rename(&temp_dir, &options.doc_embedding_dir)?;

    Ok(ThirawatDocEmbeddingBuildSummary {
        doc_embedding_artifact_id: artifact_id,
        document_count: documents.len(),
        token_count,
        dimension,
        manifest_path: options
            .doc_embedding_dir
            .join("manifest.json")
            .display()
            .to_string(),
    })
}

fn validate_documents(documents: &[ThirawatDocEmbeddingDocument]) -> Result<(usize, usize)> {
    let mut token_count = 0;
    let mut dimension = None;
    for document in documents {
        if document.concept.domain_id != "Drug" || document.concept.standard_concept != "S" {
            return Err(UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                "THIRAWAT document embeddings can only include standard Drug concepts",
            ));
        }
        if document.token_vectors.is_empty() {
            return Err(UsagiError::new(
                ErrorCode::EmbeddingFailed,
                format!(
                    "document {} has no token vectors",
                    document.concept.concept_id
                ),
            ));
        }
        if document.token_ids.len() != document.token_vectors.len() {
            return Err(UsagiError::new(
                ErrorCode::EmbeddingFailed,
                format!(
                    "document {} token_ids length does not match token_vectors length",
                    document.concept.concept_id
                ),
            ));
        }
        for vector in &document.token_vectors {
            if vector.is_empty() {
                return Err(UsagiError::new(
                    ErrorCode::EmbeddingFailed,
                    "THIRAWAT token vectors must not be empty",
                ));
            }
            if let Some(expected) = dimension {
                if vector.len() != expected {
                    return Err(UsagiError::new(
                        ErrorCode::EmbeddingFailed,
                        "THIRAWAT token vectors have inconsistent dimensions",
                    ));
                }
            } else {
                dimension = Some(vector.len());
            }
        }
        token_count += document.token_vectors.len();
    }
    Ok((
        token_count,
        dimension.ok_or_else(|| {
            UsagiError::new(
                ErrorCode::EmbeddingFailed,
                "at least one THIRAWAT document embedding is required",
            )
        })?,
    ))
}

fn manifest_outputs(dir: &std::path::Path) -> Result<Vec<ManifestFile>> {
    [
        "token_vectors.npy",
        "token_ids.npy",
        "doclens.npy",
        "doc_ids.arrow",
    ]
    .into_iter()
    .map(|path| {
        Ok(ManifestFile {
            path: path.to_string(),
            sha256: Some(sha256_file(dir.join(path))?),
            content_type: Some("application/octet-stream".to_string()),
        })
    })
    .collect()
}

fn write_npy_f32_2d(
    path: impl AsRef<std::path::Path>,
    values: &[f32],
    rows: usize,
    cols: usize,
) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    write_npy_header(&mut file, "<f4", &[rows, cols])?;
    for value in values {
        file.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

fn write_npy_i64_1d(path: impl AsRef<std::path::Path>, values: &[i64]) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    write_npy_header(&mut file, "<i8", &[values.len()])?;
    for value in values {
        file.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

fn write_npy_header(mut writer: impl Write, dtype: &str, shape: &[usize]) -> Result<()> {
    let shape = match shape {
        [one] => format!("({},)", one),
        [rows, cols] => format!("({}, {})", rows, cols),
        _ => {
            return Err(UsagiError::bad_request(
                "minimal NPY writer supports only 1D and 2D arrays",
            ))
        }
    };
    let mut header = format!(
        "{{'descr': '{}', 'fortran_order': False, 'shape': {}, }}",
        dtype, shape
    );
    let preamble_len = 10;
    let padding = (16 - ((preamble_len + header.len() + 1) % 16)) % 16;
    header.extend(std::iter::repeat_n(' ', padding));
    header.push('\n');
    writer.write_all(b"\x93NUMPY")?;
    writer.write_all(&[1, 0])?;
    writer.write_all(&(header.len() as u16).to_le_bytes())?;
    writer.write_all(header.as_bytes())?;
    Ok(())
}
