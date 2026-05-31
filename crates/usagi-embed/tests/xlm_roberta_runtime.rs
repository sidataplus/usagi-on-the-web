use std::fs;

use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};
use candle_transformers::models::xlm_roberta::{Config, XLMRobertaModel};
use tempfile::TempDir;
use tokenizers::models::wordlevel::WordLevel;
use tokenizers::pre_tokenizers::whitespace::Whitespace;
use tokenizers::Tokenizer;
use usagi_embed::xlm_roberta::{
    encode_projected_tokens, encode_sapbert_cls, XlmRobertaEncodeOptions,
    XlmRobertaTokenEncodeOptions,
};

#[test]
fn sapbert_cls_encoder_loads_local_xlm_roberta_artifact_and_normalizes_cls() {
    let dir = tiny_xlm_roberta_artifact();

    let embeddings = encode_sapbert_cls(
        XlmRobertaEncodeOptions {
            model_dir: dir.path().to_path_buf(),
            max_length: 4,
        },
        &["alpha beta"],
    )
    .expect("tiny model should encode");

    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].token_ids, vec![2, 3]);
    assert_eq!(embeddings[0].attention_mask, vec![1, 1]);
    assert_eq!(embeddings[0].vector.len(), 4);
    assert!(embeddings[0].vector.iter().all(|value| value.is_finite()));

    let norm = embeddings[0]
        .vector
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    assert!((norm - 1.0).abs() < 1e-5, "CLS vector norm was {norm}");
}

#[test]
fn token_encoder_applies_projection_and_normalizes_each_token_vector() {
    let dir = tiny_xlm_roberta_artifact();
    write_tiny_projection(&dir, 2);

    let embeddings = encode_projected_tokens(
        XlmRobertaTokenEncodeOptions {
            model_dir: dir.path().to_path_buf(),
            projection_safetensors: dir.path().join("colbert_projection.safetensors"),
            max_length: 4,
            output_dim: 2,
        },
        &["alpha beta"],
    )
    .expect("tiny projected encoder should encode");

    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].token_ids, vec![2, 3]);
    assert_eq!(embeddings[0].attention_mask, vec![true, true]);
    assert_eq!(embeddings[0].vectors.len(), 2);
    assert!(embeddings[0]
        .vectors
        .iter()
        .all(|vector| vector.len() == 2 && vector.iter().all(|value| value.is_finite())));
    for vector in &embeddings[0].vectors {
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "token vector norm was {norm}");
    }
}

fn tiny_xlm_roberta_artifact() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_json = serde_json::json!({
        "hidden_size": 4,
        "layer_norm_eps": 1e-5,
        "attention_probs_dropout_prob": 0.0,
        "hidden_dropout_prob": 0.0,
        "num_attention_heads": 1,
        "position_embedding_type": "absolute",
        "intermediate_size": 4,
        "hidden_act": "gelu",
        "num_hidden_layers": 1,
        "vocab_size": 4,
        "max_position_embeddings": 8,
        "type_vocab_size": 1,
        "pad_token_id": 0
    });
    fs::write(
        dir.path().join("config.json"),
        serde_json::to_vec_pretty(&config_json).expect("config json"),
    )
    .expect("write config");
    fs::write(dir.path().join("manifest.json"), "{}").expect("manifest");

    let config: Config = serde_json::from_value(config_json).expect("config");
    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let _model = XLMRobertaModel::new(&config, vb.pp("roberta")).expect("init tiny model");
    varmap
        .save(dir.path().join("model.safetensors"))
        .expect("save model");

    let vocab_path = dir.path().join("vocab.json");
    fs::write(&vocab_path, r#"{"<pad>":0,"<unk>":1,"alpha":2,"beta":3}"#).expect("vocab");
    let word_level = WordLevel::from_file(
        vocab_path.to_str().expect("vocab path"),
        "<unk>".to_string(),
    )
    .expect("wordlevel");
    let mut tokenizer = Tokenizer::new(word_level);
    tokenizer.with_pre_tokenizer(Some(Whitespace));
    tokenizer
        .save(dir.path().join("tokenizer.json"), true)
        .expect("tokenizer");

    dir
}

fn write_tiny_projection(dir: &TempDir, output_dim: usize) {
    let device = Device::Cpu;
    let weight = Tensor::from_vec(
        vec![
            1.0_f32, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0,
        ],
        (output_dim, 4),
        &device,
    )
    .expect("projection tensor");
    let mut tensors = std::collections::HashMap::new();
    tensors.insert("weight".to_string(), weight);
    candle_core::safetensors::save(&tensors, dir.path().join("colbert_projection.safetensors"))
        .expect("projection safetensors");
}
