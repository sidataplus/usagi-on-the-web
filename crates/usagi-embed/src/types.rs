use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClsEmbedding {
    pub text: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenEmbedding {
    pub text: String,
    pub token_ids: Vec<i64>,
    pub vectors: Vec<Vec<f32>>,
    pub attention_mask: Vec<bool>,
}
