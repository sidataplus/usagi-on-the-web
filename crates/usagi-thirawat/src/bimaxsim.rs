use usagi_common::error::{ErrorCode, Result, UsagiError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiMaxSimScore {
    pub query_to_document: f32,
    pub document_to_query: f32,
    pub score: f32,
}

pub fn exact_bimaxsim(
    query_vectors: &[Vec<f32>],
    document_vectors: &[Vec<f32>],
) -> Result<BiMaxSimScore> {
    validate_matrix("query_vectors", query_vectors)?;
    validate_matrix("document_vectors", document_vectors)?;
    let query_to_document = directional_maxsim(query_vectors, document_vectors);
    let document_to_query = directional_maxsim(document_vectors, query_vectors);
    Ok(BiMaxSimScore {
        query_to_document,
        document_to_query,
        score: (query_to_document + document_to_query) / 2.0,
    })
}

fn directional_maxsim(source: &[Vec<f32>], target: &[Vec<f32>]) -> f32 {
    let total: f32 = source
        .iter()
        .map(|source_vector| {
            target
                .iter()
                .map(|target_vector| cosine(source_vector, target_vector))
                .fold(f32::NEG_INFINITY, f32::max)
        })
        .sum();
    total / source.len() as f32
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (left_value, right_value) in left.iter().zip(right.iter()) {
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }
    dot / (left_norm.sqrt() * right_norm.sqrt())
}

fn validate_matrix(name: &str, matrix: &[Vec<f32>]) -> Result<()> {
    let Some(first) = matrix.first() else {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!("{name} must contain at least one token vector"),
        ));
    };
    if first.is_empty() {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!("{name} token vectors must not be empty"),
        ));
    }
    for row in matrix {
        if row.len() != first.len() {
            return Err(UsagiError::new(
                ErrorCode::EmbeddingFailed,
                format!("{name} token vectors have inconsistent dimensions"),
            ));
        }
    }
    Ok(())
}
