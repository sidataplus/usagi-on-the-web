use std::path::Path;

use usagi_common::error::{ErrorCode, Result, UsagiError};

use crate::types::{ClsEmbedding, TokenEmbedding};

pub fn l2_normalize(vector: &[f32]) -> Result<Vec<f32>> {
    if vector.is_empty() {
        return Err(UsagiError::bad_request("cannot normalize an empty vector"));
    }
    let norm = vector
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt();
    if norm == 0.0 {
        return Err(UsagiError::bad_request("cannot normalize a zero vector"));
    }
    Ok(vector
        .iter()
        .map(|value| (*value as f64 / norm) as f32)
        .collect())
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32> {
    if left.len() != right.len() {
        return Err(UsagiError::bad_request(format!(
            "cosine vectors have different dimensions: {} vs {}",
            left.len(),
            right.len()
        )));
    }
    if left.is_empty() {
        return Err(UsagiError::bad_request("cosine vectors must not be empty"));
    }
    let left_norm = left
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt();
    let right_norm = right
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        return Err(UsagiError::bad_request(
            "cosine vectors must not be zero vectors",
        ));
    }
    let dot = left
        .iter()
        .zip(right.iter())
        .map(|(left, right)| (*left as f64) * (*right as f64))
        .sum::<f64>();
    Ok((dot / (left_norm * right_norm)) as f32)
}

pub fn assert_cls_parity(
    candidate: &ClsEmbedding,
    reference: &ClsEmbedding,
    min_cosine: f32,
) -> Result<()> {
    if candidate.text != reference.text {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!(
                "CLS parity text mismatch: candidate {:?}, reference {:?}",
                candidate.text, reference.text
            ),
        ));
    }
    let cosine = cosine_similarity(&candidate.vector, &reference.vector)?;
    if cosine < min_cosine {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!("CLS parity cosine {cosine:.6} below required {min_cosine:.6}"),
        ));
    }
    Ok(())
}

pub fn assert_cls_parity_files(
    candidate_path: impl AsRef<Path>,
    reference_path: impl AsRef<Path>,
    min_cosine: f32,
) -> Result<()> {
    let candidate = read_cls_embedding(candidate_path.as_ref())?;
    let reference = read_cls_embedding(reference_path.as_ref())?;
    assert_cls_parity(&candidate, &reference, min_cosine)
}

pub fn assert_token_parity(
    candidate: &TokenEmbedding,
    reference: &TokenEmbedding,
    min_mean_cosine: f32,
    max_length: usize,
    output_dim: usize,
) -> Result<()> {
    if candidate.text != reference.text {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!(
                "token parity text mismatch: candidate {:?}, reference {:?}",
                candidate.text, reference.text
            ),
        ));
    }
    if candidate.token_ids != reference.token_ids {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            "token parity token_ids mismatch",
        ));
    }
    if candidate.attention_mask != reference.attention_mask {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            "token parity attention_mask mismatch",
        ));
    }
    validate_token_embedding_shape(candidate, max_length, output_dim)?;
    validate_token_embedding_shape(reference, max_length, output_dim)?;
    if candidate.vectors.len() != reference.vectors.len() {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!(
                "token parity vector length mismatch: {} vs {}",
                candidate.vectors.len(),
                reference.vectors.len()
            ),
        ));
    }
    let mean_cosine = candidate
        .vectors
        .iter()
        .zip(reference.vectors.iter())
        .map(|(candidate_vector, reference_vector)| {
            cosine_similarity(candidate_vector, reference_vector).map(|value| value as f64)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .sum::<f64>()
        / candidate.vectors.len() as f64;
    if mean_cosine < min_mean_cosine as f64 {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!(
                "token parity mean token cosine {mean_cosine:.6} below required {min_mean_cosine:.6}"
            ),
        ));
    }
    Ok(())
}

pub fn assert_token_parity_files(
    candidate_path: impl AsRef<Path>,
    reference_path: impl AsRef<Path>,
    min_mean_cosine: f32,
    max_length: usize,
    output_dim: usize,
) -> Result<()> {
    let candidate = read_token_embedding(candidate_path.as_ref())?;
    let reference = read_token_embedding(reference_path.as_ref())?;
    assert_token_parity(
        &candidate,
        &reference,
        min_mean_cosine,
        max_length,
        output_dim,
    )
}

pub fn read_cls_embedding(path: &Path) -> Result<ClsEmbedding> {
    serde_json::from_slice(&std::fs::read(path)?).map_err(|err| {
        UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "CLS parity fixture {} is invalid JSON: {err}",
                path.display()
            ),
        )
    })
}

pub fn read_token_embedding(path: &Path) -> Result<TokenEmbedding> {
    serde_json::from_slice(&std::fs::read(path)?).map_err(|err| {
        UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "token parity fixture {} is invalid JSON: {err}",
                path.display()
            ),
        )
    })
}

fn validate_token_embedding_shape(
    embedding: &TokenEmbedding,
    max_length: usize,
    output_dim: usize,
) -> Result<()> {
    if embedding.token_ids.len() > max_length {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!(
                "token parity sequence length {} exceeds max_length {}",
                embedding.token_ids.len(),
                max_length
            ),
        ));
    }
    if embedding.attention_mask.len() != embedding.token_ids.len() {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!(
                "token parity attention_mask length {} does not match token_ids length {}",
                embedding.attention_mask.len(),
                embedding.token_ids.len()
            ),
        ));
    }
    if embedding.vectors.len() != embedding.token_ids.len() {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!(
                "token parity vector length {} does not match token_ids length {}",
                embedding.vectors.len(),
                embedding.token_ids.len()
            ),
        ));
    }
    if embedding.vectors.is_empty() {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            "token parity vectors must not be empty",
        ));
    }
    for vector in &embedding.vectors {
        if vector.len() != output_dim {
            return Err(UsagiError::new(
                ErrorCode::EmbeddingFailed,
                format!(
                    "token parity output dimension {} does not match expected {}",
                    vector.len(),
                    output_dim
                ),
            ));
        }
    }
    Ok(())
}
