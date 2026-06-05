use std::fs;

use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::{Linear, Module, VarBuilder};
use candle_transformers::models::xlm_roberta::{Config, XLMRobertaModel};
use serde::Serialize;
use tokenizers::Tokenizer;
use usagi_common::error::{ErrorCode, Result, UsagiError};

use crate::artifact::{validate_sapbert_model_artifact, SapbertModelArtifactPaths};
use crate::parity::l2_normalize;
use crate::types::TokenEmbedding;

#[derive(Debug, Clone)]
pub struct XlmRobertaEncodeOptions {
    pub model_dir: std::path::PathBuf,
    pub max_length: usize,
}

#[derive(Debug, Clone)]
pub struct XlmRobertaTokenEncodeOptions {
    pub model_dir: std::path::PathBuf,
    pub projection_safetensors: std::path::PathBuf,
    pub max_length: usize,
    pub output_dim: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClsEmbedding {
    pub text: String,
    pub token_ids: Vec<u32>,
    pub attention_mask: Vec<u32>,
    pub vector: Vec<f32>,
}

pub fn encode_sapbert_cls(
    options: XlmRobertaEncodeOptions,
    texts: &[impl AsRef<str>],
) -> Result<Vec<ClsEmbedding>> {
    encode_sapbert_cls_with_progress(options, texts, |_processed, _total| Ok(()))
}

pub fn encode_sapbert_cls_with_progress(
    options: XlmRobertaEncodeOptions,
    texts: &[impl AsRef<str>],
    mut progress: impl FnMut(usize, usize) -> Result<()>,
) -> Result<Vec<ClsEmbedding>> {
    if options.max_length == 0 {
        return Err(UsagiError::bad_request(
            "SapBERT max_length must be greater than zero",
        ));
    }
    let artifact = validate_sapbert_model_artifact(SapbertModelArtifactPaths {
        model_dir: options.model_dir,
    })?;
    let tokenizer = Tokenizer::from_file(&artifact.tokenizer_json).map_err(|err| {
        UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "SapBERT tokenizer {} is invalid: {err}",
                artifact.tokenizer_json.display()
            ),
        )
    })?;
    let config: Config =
        serde_json::from_slice(&fs::read(&artifact.config_json)?).map_err(|err| {
            UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                format!(
                    "SapBERT config {} is invalid: {err}",
                    artifact.config_json.display()
                ),
            )
        })?;
    let device = Device::Cpu;
    let model = load_xlm_roberta_model(&artifact.model_safetensors, &config, &device)?;

    let total = texts.len();
    let mut embeddings = Vec::with_capacity(total);
    for (index, text) in texts.iter().enumerate() {
        embeddings.push(encode_one(
            &model,
            &tokenizer,
            &device,
            text.as_ref(),
            options.max_length,
        )?);
        progress(index + 1, total)?;
    }
    Ok(embeddings)
}

pub fn encode_projected_tokens(
    options: XlmRobertaTokenEncodeOptions,
    texts: &[impl AsRef<str>],
) -> Result<Vec<TokenEmbedding>> {
    encode_projected_tokens_with_progress(options, texts, |_processed, _total| Ok(()))
}

pub fn encode_projected_tokens_with_progress(
    options: XlmRobertaTokenEncodeOptions,
    texts: &[impl AsRef<str>],
    mut progress: impl FnMut(usize, usize) -> Result<()>,
) -> Result<Vec<TokenEmbedding>> {
    if options.max_length == 0 {
        return Err(UsagiError::bad_request(
            "XLM-R token max_length must be greater than zero",
        ));
    }
    for path in [
        options.model_dir.join("config.json"),
        options.model_dir.join("tokenizer.json"),
        options.model_dir.join("model.safetensors"),
        options.projection_safetensors.clone(),
    ] {
        if !path.exists() {
            return Err(UsagiError::new(
                ErrorCode::ModelNotReady,
                format!("XLM-R token encoder artifact is missing {}", path.display()),
            ));
        }
    }
    let tokenizer_path = options.model_dir.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|err| {
        UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "XLM-R tokenizer {} is invalid: {err}",
                tokenizer_path.display()
            ),
        )
    })?;
    let config_path = options.model_dir.join("config.json");
    let config: Config = serde_json::from_slice(&fs::read(&config_path)?).map_err(|err| {
        UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!("XLM-R config {} is invalid: {err}", config_path.display()),
        )
    })?;
    let device = Device::Cpu;
    let model = load_xlm_roberta_model(
        &options.model_dir.join("model.safetensors"),
        &config,
        &device,
    )?;
    let projection = load_projection(
        &options.projection_safetensors,
        config.hidden_size,
        options.output_dim,
        &device,
    )?;

    let total = texts.len();
    let mut embeddings = Vec::with_capacity(total);
    for (index, text) in texts.iter().enumerate() {
        embeddings.push(encode_projected_token_one(
            &model,
            &projection,
            &tokenizer,
            &device,
            text.as_ref(),
            options.max_length,
        )?);
        progress(index + 1, total)?;
    }
    Ok(embeddings)
}

fn load_xlm_roberta_model(
    safetensors_path: &std::path::Path,
    config: &Config,
    device: &Device,
) -> Result<XLMRobertaModel> {
    let data = fs::read(safetensors_path)?;
    let vb = VarBuilder::from_buffered_safetensors(data.clone(), DType::F32, device)
        .map_err(candle_error)?;
    match XLMRobertaModel::new(config, vb.pp("roberta")) {
        Ok(model) => Ok(model),
        Err(prefix_err) => {
            let vb = VarBuilder::from_buffered_safetensors(data, DType::F32, device)
                .map_err(candle_error)?;
            XLMRobertaModel::new(config, vb).map_err(|err| {
                UsagiError::new(
                    ErrorCode::IncompatibleArtifact,
                    format!(
                        "SapBERT XLM-R weights could not be loaded with or without roberta prefix: {prefix_err}; {err}"
                    ),
                )
            })
        }
    }
}

fn load_projection(
    safetensors_path: &std::path::Path,
    hidden_dim: usize,
    output_dim: usize,
    device: &Device,
) -> Result<Linear> {
    let tensors = candle_core::safetensors::load(safetensors_path, device).map_err(candle_error)?;
    let preferred_names = [
        "weight",
        "linear.weight",
        "dense.weight",
        "projection.weight",
        "colbert_projection.weight",
    ];
    let weight = preferred_names
        .iter()
        .find_map(|name| tensors.get(*name).cloned())
        .or_else(|| {
            let mut keys = tensors.keys().collect::<Vec<_>>();
            keys.sort();
            keys.into_iter().find_map(|key| {
                tensors
                    .get(key)
                    .and_then(|tensor| tensor.dims2().ok().map(|_| tensor.clone()))
            })
        })
        .ok_or_else(|| {
            UsagiError::new(
                ErrorCode::IncompatibleArtifact,
                format!(
                    "projection safetensors {} does not contain a 2D weight tensor",
                    safetensors_path.display()
                ),
            )
        })?;
    let (rows, cols) = weight.dims2().map_err(candle_error)?;
    if rows != output_dim || cols != hidden_dim {
        return Err(UsagiError::new(
            ErrorCode::IncompatibleArtifact,
            format!(
                "projection weight shape [{rows}, {cols}] does not match expected [{output_dim}, {hidden_dim}]"
            ),
        ));
    }
    Ok(Linear::new(weight, None))
}

fn encode_one(
    model: &XLMRobertaModel,
    tokenizer: &Tokenizer,
    device: &Device,
    text: &str,
    max_length: usize,
) -> Result<ClsEmbedding> {
    let encoding = tokenizer.encode(text, true).map_err(|err| {
        UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!("SapBERT tokenization failed for {text:?}: {err}"),
        )
    })?;
    let mut token_ids = encoding.get_ids().to_vec();
    if token_ids.is_empty() {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!("SapBERT tokenizer produced no tokens for {text:?}"),
        ));
    }
    token_ids.truncate(max_length);
    let attention_mask = vec![1_u32; token_ids.len()];
    let token_type_ids = vec![0_u32; token_ids.len()];
    let attention_mask_f32 = vec![1_f32; token_ids.len()];

    let input_ids = Tensor::new(token_ids.as_slice(), device)
        .map_err(candle_error)?
        .unsqueeze(0)
        .map_err(candle_error)?;
    let attention_mask_tensor = Tensor::new(attention_mask_f32.as_slice(), device)
        .map_err(candle_error)?
        .unsqueeze(0)
        .map_err(candle_error)?;
    let token_type_ids = Tensor::new(token_type_ids.as_slice(), device)
        .map_err(candle_error)?
        .unsqueeze(0)
        .map_err(candle_error)?;

    let hidden = model
        .forward(
            &input_ids,
            &attention_mask_tensor,
            &token_type_ids,
            None,
            None,
            None,
        )
        .map_err(candle_error)?;
    let cls = hidden
        .i((0, 0))
        .map_err(candle_error)?
        .to_vec1::<f32>()
        .map_err(candle_error)?;

    Ok(ClsEmbedding {
        text: text.to_string(),
        token_ids,
        attention_mask,
        vector: l2_normalize(&cls)?,
    })
}

fn encode_projected_token_one(
    model: &XLMRobertaModel,
    projection: &Linear,
    tokenizer: &Tokenizer,
    device: &Device,
    text: &str,
    max_length: usize,
) -> Result<TokenEmbedding> {
    let encoding = tokenizer.encode(text, true).map_err(|err| {
        UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!("XLM-R tokenization failed for {text:?}: {err}"),
        )
    })?;
    let mut token_ids = encoding.get_ids().to_vec();
    if token_ids.is_empty() {
        return Err(UsagiError::new(
            ErrorCode::EmbeddingFailed,
            format!("XLM-R tokenizer produced no tokens for {text:?}"),
        ));
    }
    token_ids.truncate(max_length);
    let token_type_ids = vec![0_u32; token_ids.len()];
    let attention_mask = vec![true; token_ids.len()];
    let attention_mask_f32 = vec![1_f32; token_ids.len()];

    let input_ids = Tensor::new(token_ids.as_slice(), device)
        .map_err(candle_error)?
        .unsqueeze(0)
        .map_err(candle_error)?;
    let attention_mask_tensor = Tensor::new(attention_mask_f32.as_slice(), device)
        .map_err(candle_error)?
        .unsqueeze(0)
        .map_err(candle_error)?;
    let token_type_ids = Tensor::new(token_type_ids.as_slice(), device)
        .map_err(candle_error)?
        .unsqueeze(0)
        .map_err(candle_error)?;

    let hidden = model
        .forward(
            &input_ids,
            &attention_mask_tensor,
            &token_type_ids,
            None,
            None,
            None,
        )
        .map_err(candle_error)?;
    let projected = projection.forward(&hidden).map_err(candle_error)?;
    let vectors = projected
        .squeeze(0)
        .map_err(candle_error)?
        .to_vec2::<f32>()
        .map_err(candle_error)?
        .into_iter()
        .map(|vector| l2_normalize(&vector))
        .collect::<Result<Vec<_>>>()?;

    Ok(TokenEmbedding {
        text: text.to_string(),
        token_ids: token_ids.into_iter().map(i64::from).collect(),
        vectors,
        attention_mask,
    })
}

fn candle_error(err: candle_core::Error) -> UsagiError {
    UsagiError::new(ErrorCode::EmbeddingFailed, err.to_string())
}
