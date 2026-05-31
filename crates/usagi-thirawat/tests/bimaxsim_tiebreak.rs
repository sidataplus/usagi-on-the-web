use usagi_common::error::ErrorCode;
use usagi_common::manifest::sha256_file;
use usagi_thirawat::artifact::{
    validate_tachiom_artifact, validate_thirawat_doc_embedding_artifact,
    validate_thirawat_model_artifact, TachiomArtifactPaths, ThirawatDocEmbeddingArtifactPaths,
    ThirawatModelArtifactPaths,
};
use usagi_thirawat::bimaxsim::exact_bimaxsim;
use usagi_thirawat::tiebreak::{rank_near_ties, TieBreakCandidate, TieBreakOptions};

#[test]
fn exact_bimaxsim_averages_both_token_coverage_directions() {
    let query = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
    let document = vec![vec![1.0, 0.0], vec![1.0, 0.0]];

    let score = exact_bimaxsim(&query, &document).expect("valid vectors");

    assert!((score.query_to_document - 0.5).abs() < 0.0001);
    assert!((score.document_to_query - 1.0).abs() < 0.0001);
    assert!((score.score - 0.75).abs() < 0.0001);
}

#[test]
fn exact_bimaxsim_rejects_inconsistent_dimensions() {
    let err = exact_bimaxsim(&[vec![1.0], vec![1.0, 0.0]], &[vec![1.0]])
        .expect_err("inconsistent dimensions should fail");

    assert_eq!(err.code(), ErrorCode::EmbeddingFailed);
}

#[test]
fn tiebreak_prefers_matching_strength_and_form_inside_epsilon() {
    let ranked = rank_near_ties(
        "amoxicillin clavulanate 875 mg tablet",
        vec![
            TieBreakCandidate {
                concept_id: 2,
                concept_name: "Amoxicillin / Clavulanate 500 MG Oral Tablet".to_string(),
                bimaxsim: 0.914,
                tachiom_maxsim: 0.88,
            },
            TieBreakCandidate {
                concept_id: 3,
                concept_name: "Amoxicillin / Clavulanate 875 MG Oral Capsule".to_string(),
                bimaxsim: 0.913,
                tachiom_maxsim: 0.88,
            },
            TieBreakCandidate {
                concept_id: 1,
                concept_name: "Amoxicillin / Clavulanate 875 MG Oral Tablet".to_string(),
                bimaxsim: 0.912,
                tachiom_maxsim: 0.87,
            },
        ],
        TieBreakOptions {
            epsilon: 0.01,
            top_n: 100,
        },
    );

    assert_eq!(ranked[0].concept_id, 1);
    assert_eq!(ranked[0].features.strength_exact, Some(true));
    assert_eq!(ranked[0].features.dose_form_match, Some(true));
}

#[test]
fn tiebreak_does_not_reorder_outside_epsilon() {
    let ranked = rank_near_ties(
        "amoxicillin clavulanate 875 mg tablet",
        vec![
            TieBreakCandidate {
                concept_id: 10,
                concept_name: "Amoxicillin / Clavulanate 500 MG Oral Capsule".to_string(),
                bimaxsim: 0.95,
                tachiom_maxsim: 0.88,
            },
            TieBreakCandidate {
                concept_id: 1,
                concept_name: "Amoxicillin / Clavulanate 875 MG Oral Tablet".to_string(),
                bimaxsim: 0.90,
                tachiom_maxsim: 0.87,
            },
        ],
        TieBreakOptions {
            epsilon: 0.01,
            top_n: 100,
        },
    );

    assert_eq!(ranked[0].concept_id, 10);
}

#[test]
fn artifact_validators_require_documented_mapper_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let model_dir = temp.path().join("model");
    let doc_dir = temp.path().join("doc_embeddings");
    let tachiom_dir = temp.path().join("tachiom");
    std::fs::create_dir_all(&model_dir).expect("model dir");
    std::fs::create_dir_all(&doc_dir).expect("doc dir");
    std::fs::create_dir_all(&tachiom_dir).expect("tachiom dir");

    assert_eq!(
        validate_thirawat_model_artifact(ThirawatModelArtifactPaths {
            model_dir: model_dir.clone()
        })
        .expect_err("missing model")
        .code(),
        ErrorCode::ModelNotReady
    );
    assert_eq!(
        validate_thirawat_doc_embedding_artifact(ThirawatDocEmbeddingArtifactPaths {
            doc_embedding_dir: doc_dir.clone()
        })
        .expect_err("missing doc embeddings")
        .code(),
        ErrorCode::IndexNotReady
    );
    assert_eq!(
        validate_tachiom_artifact(TachiomArtifactPaths {
            index_dir: tachiom_dir.clone()
        })
        .expect_err("missing tachiom index")
        .code(),
        ErrorCode::IndexNotReady
    );

    for filename in [
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
        "model.safetensors",
        "colbert_projection.safetensors",
        "manifest.json",
    ] {
        std::fs::write(model_dir.join(filename), b"{}").expect("model file");
    }
    std::fs::write(
        model_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "model_id": "sidataplus/THIRAWAT-SapBERT",
            "base_model": "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR",
            "architecture": "pylate_colbert",
            "encoder_family": "xlm-roberta",
            "query_length": 96,
            "document_length": 96,
            "hidden_dim": 768,
            "projection_dim": 128,
            "similarity": "maxsim",
            "projection": {
                "in_features": 768,
                "out_features": 128,
                "bias": false
            },
            "peft": {
                "merged": true
            },
            "sha256": {
                "model.safetensors": sha256_file(model_dir.join("model.safetensors")).unwrap(),
                "colbert_projection.safetensors": sha256_file(model_dir.join("colbert_projection.safetensors")).unwrap()
            }
        }))
        .expect("manifest json"),
    )
    .expect("manifest file");
    for filename in [
        "token_vectors.npy",
        "token_ids.npy",
        "doclens.npy",
        "doc_ids.arrow",
        "manifest.json",
    ] {
        std::fs::write(doc_dir.join(filename), b"{}").expect("doc file");
    }
    for filename in ["index.bin", "manifest.json"] {
        std::fs::write(tachiom_dir.join(filename), b"{}").expect("tachiom file");
    }

    validate_thirawat_model_artifact(ThirawatModelArtifactPaths { model_dir })
        .expect("complete model accepted");
    validate_thirawat_doc_embedding_artifact(ThirawatDocEmbeddingArtifactPaths {
        doc_embedding_dir: doc_dir,
    })
    .expect("complete doc embeddings accepted");
    validate_tachiom_artifact(TachiomArtifactPaths {
        index_dir: tachiom_dir,
    })
    .expect("complete tachiom index accepted");
}
