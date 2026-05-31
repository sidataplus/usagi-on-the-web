use serde_json::json;
use usagi_contracts::catalog::ConceptSummary;
use usagi_contracts::mapper::{MapperDrugBatchItemRequest, MapperDrugBatchRequest};
use usagi_thirawat::mapper::{
    map_drug_batch_from_precomputed, map_drug_explain_from_precomputed,
    map_drug_query_from_precomputed, map_drug_query_with_vectors, MapperRuntimeOptions,
    PrecomputedQueryEmbedding, PrecomputedQueryEmbeddings, TachiomFixtureDocument,
    TachiomFixtureIndex,
};

#[test]
fn mapper_pipeline_retrieves_reranks_and_tiebreaks_from_tachiom_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let index_dir = dir.path().join("tachiom");
    std::fs::create_dir_all(&index_dir).unwrap();

    let fixture = TachiomFixtureIndex {
        documents: vec![
            TachiomFixtureDocument {
                concept: concept(
                    2,
                    "Amoxicillin / Clavulanate 500 MG Oral Tablet",
                    "Clinical Drug",
                ),
                token_vectors: vec![vec![1.0, 0.0], vec![0.7, 0.7]],
            },
            TachiomFixtureDocument {
                concept: concept(
                    1,
                    "Amoxicillin / Clavulanate 875 MG Oral Tablet",
                    "Clinical Drug",
                ),
                token_vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            },
            TachiomFixtureDocument {
                concept: concept(
                    3,
                    "Amoxicillin / Clavulanate 875 MG Oral Capsule",
                    "Clinical Drug",
                ),
                token_vectors: vec![vec![1.0, 0.0], vec![0.1, 0.9]],
            },
        ],
    };
    std::fs::write(
        index_dir.join("index.bin"),
        serde_json::to_vec_pretty(&fixture).unwrap(),
    )
    .unwrap();
    std::fs::write(index_dir.join("manifest.json"), "{}").unwrap();

    let response = map_drug_query_with_vectors(
        "amoxicillin clavulanate 875 mg tablet",
        Some("SRC001"),
        vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        &index_dir,
        MapperRuntimeOptions {
            candidate_top_k: 3,
            rerank_top_n: 3,
            limit: 2,
            epsilon: 0.01,
            tiebreak_top_n: 100,
        },
    )
    .unwrap();

    assert_eq!(response.mode, "thirawat_tachiom");
    assert_eq!(response.candidates.len(), 2);
    assert_eq!(response.candidates[0].rank, 1);
    assert_eq!(response.candidates[0].concept.concept_id, 1);
    assert_eq!(
        response.candidates[0].method,
        "thirawat_tachiom_bimaxsim_tiebreak"
    );
    assert_eq!(response.candidates[0].features["strength_exact"], true);
    assert_eq!(response.candidates[0].features["dose_form_match"], true);
    assert!(response.candidates[0].scores["bimaxsim"].as_f64().unwrap() > 0.99);
    assert_eq!(
        response.query,
        json!({
            "source_name": "amoxicillin clavulanate 875 mg tablet",
            "source_code": "SRC001",
            "query_text": "amoxicillin clavulanate 875 mg tablet (SRC001)"
        })
    );
}

#[test]
fn mapper_pipeline_uses_precomputed_query_embedding_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let index_dir = dir.path().join("tachiom");
    std::fs::create_dir_all(&index_dir).unwrap();
    std::fs::write(
        index_dir.join("index.bin"),
        serde_json::to_vec_pretty(&TachiomFixtureIndex {
            documents: vec![TachiomFixtureDocument {
                concept: concept(
                    1,
                    "Amoxicillin / Clavulanate 875 MG Oral Tablet",
                    "Clinical Drug",
                ),
                token_vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            }],
        })
        .unwrap(),
    )
    .unwrap();
    std::fs::write(index_dir.join("manifest.json"), "{}").unwrap();

    let query_path = dir.path().join("query_embeddings.json");
    std::fs::write(
        &query_path,
        serde_json::to_vec_pretty(&PrecomputedQueryEmbeddings {
            items: vec![PrecomputedQueryEmbedding {
                source_name: "amoxicillin clavulanate 875 mg tablet".to_string(),
                source_code: Some("SRC001".to_string()),
                token_vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            }],
        })
        .unwrap(),
    )
    .unwrap();

    let response = map_drug_query_from_precomputed(
        "amoxicillin clavulanate 875 mg tablet",
        Some("SRC001"),
        &query_path,
        &index_dir,
        MapperRuntimeOptions::default(),
    )
    .unwrap();

    assert_eq!(response.candidates[0].concept.concept_id, 1);
}

#[test]
fn mapper_explain_returns_scores_features_and_token_debug_for_requested_concept() {
    let dir = tempfile::tempdir().unwrap();
    let index_dir = dir.path().join("tachiom");
    std::fs::create_dir_all(&index_dir).unwrap();
    std::fs::write(
        index_dir.join("index.bin"),
        serde_json::to_vec_pretty(&TachiomFixtureIndex {
            documents: vec![TachiomFixtureDocument {
                concept: concept(1, "Aspirin 81 MG Oral Tablet", "Clinical Drug"),
                token_vectors: vec![vec![1.0, 0.0]],
            }],
        })
        .unwrap(),
    )
    .unwrap();
    std::fs::write(index_dir.join("manifest.json"), "{}").unwrap();

    let query_path = dir.path().join("query_embeddings.json");
    std::fs::write(
        &query_path,
        serde_json::to_vec_pretty(&PrecomputedQueryEmbeddings {
            items: vec![PrecomputedQueryEmbedding {
                source_name: "aspirin 81 mg tablet".to_string(),
                source_code: Some("A".to_string()),
                token_vectors: vec![vec![1.0, 0.0]],
            }],
        })
        .unwrap(),
    )
    .unwrap();

    let explanation = map_drug_explain_from_precomputed(
        "aspirin 81 mg tablet",
        Some("A"),
        1,
        &query_path,
        &index_dir,
        MapperRuntimeOptions::default(),
    )
    .unwrap();

    assert_eq!(explanation.query["query_text"], "aspirin 81 mg tablet (A)");
    assert_eq!(explanation.concept.concept_id, 1);
    assert_eq!(explanation.scores["tachiom_maxsim"], 1.0);
    assert_eq!(explanation.scores["bimaxsim"], 1.0);
    assert_eq!(explanation.features["strength_exact"], true);
    assert_eq!(explanation.features["dose_form_match"], true);
    assert_eq!(explanation.token_debug["enabled"], false);
}

#[test]
fn mapper_batch_from_precomputed_returns_item_results_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let index_dir = dir.path().join("tachiom");
    std::fs::create_dir_all(&index_dir).unwrap();
    std::fs::write(
        index_dir.join("index.bin"),
        serde_json::to_vec_pretty(&TachiomFixtureIndex {
            documents: vec![TachiomFixtureDocument {
                concept: concept(1, "Aspirin 81 MG Oral Tablet", "Clinical Drug"),
                token_vectors: vec![vec![1.0, 0.0]],
            }],
        })
        .unwrap(),
    )
    .unwrap();
    std::fs::write(index_dir.join("manifest.json"), "{}").unwrap();

    let query_path = dir.path().join("query_embeddings.json");
    std::fs::write(
        &query_path,
        serde_json::to_vec_pretty(&PrecomputedQueryEmbeddings {
            items: vec![PrecomputedQueryEmbedding {
                source_name: "aspirin 81 mg tablet".to_string(),
                source_code: Some("A".to_string()),
                token_vectors: vec![vec![1.0, 0.0]],
            }],
        })
        .unwrap(),
    )
    .unwrap();

    let response = map_drug_batch_from_precomputed(
        MapperDrugBatchRequest {
            mode: "thirawat_tachiom".to_string(),
            candidate_top_k: 5,
            rerank_top_n: 5,
            limit: 1,
            items: vec![
                MapperDrugBatchItemRequest {
                    id: "ok".to_string(),
                    source_code: Some("A".to_string()),
                    source_name: "aspirin 81 mg tablet".to_string(),
                    source_frequency: Some(10),
                },
                MapperDrugBatchItemRequest {
                    id: "missing".to_string(),
                    source_code: Some("B".to_string()),
                    source_name: "missing query".to_string(),
                    source_frequency: None,
                },
            ],
        },
        &query_path,
        &index_dir,
    )
    .unwrap();

    assert_eq!(response.items.len(), 2);
    assert_eq!(response.items[0].id, "ok");
    assert_eq!(response.items[0].candidates[0].concept.concept_id, 1);
    assert!(response.items[0].error.is_none());
    assert_eq!(response.items[1].id, "missing");
    assert!(response.items[1].candidates.is_empty());
    assert_eq!(
        response.items[1].error.as_ref().unwrap()["code"],
        "EMBEDDING_FAILED"
    );
}

fn concept(concept_id: i64, concept_name: &str, concept_class_id: &str) -> ConceptSummary {
    ConceptSummary {
        concept_id,
        concept_name: concept_name.to_string(),
        domain_id: "Drug".to_string(),
        vocabulary_id: "RxNorm".to_string(),
        concept_class_id: concept_class_id.to_string(),
        standard_concept: "S".to_string(),
        concept_code: concept_id.to_string(),
    }
}
