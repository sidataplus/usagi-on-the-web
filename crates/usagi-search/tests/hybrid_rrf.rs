use usagi_contracts::catalog::ConceptSummary;
use usagi_contracts::search::SearchResult;
use usagi_search::hybrid::{fuse_rrf, RrfOptions};

#[test]
fn rrf_fusion_is_deterministic_and_uses_component_ranks() {
    let lexical = vec![
        result(
            1,
            100,
            "Tramadol Hydrochloride 50 MG Oral Capsule",
            "lexical_tantivy",
        ),
        result(2, 200, "Tramadol", "lexical_tantivy"),
    ];
    let dense = vec![
        result(1, 200, "Tramadol", "sapbert_cls"),
        result(
            2,
            100,
            "Tramadol Hydrochloride 50 MG Oral Capsule",
            "sapbert_cls",
        ),
    ];

    let fused = fuse_rrf(
        lexical,
        dense,
        RrfOptions {
            rrf_k: 60.0,
            limit: 2,
        },
    );

    assert_eq!(fused.len(), 2);
    assert_eq!(
        fused[0].concept.concept_id, 100,
        "tie must break deterministically by lexical rank"
    );
    assert_eq!(fused[0].method, "hybrid_rrf");
    assert_eq!(fused[0].component_ranks["tantivy"], 1);
    assert_eq!(fused[0].component_ranks["sapbert"], 2);
    assert!(fused[0].scores["rrf"].as_f64().unwrap() > 0.0);
}

#[test]
fn rrf_fusion_preserves_dense_only_candidates() {
    let lexical = vec![result(
        1,
        100,
        "Tramadol Hydrochloride 50 MG Oral Capsule",
        "lexical_tantivy",
    )];
    let dense = vec![result(1, 300, "Tramadol 50 MG Oral Tablet", "sapbert_cls")];

    let fused = fuse_rrf(
        lexical,
        dense,
        RrfOptions {
            rrf_k: 60.0,
            limit: 10,
        },
    );

    assert_eq!(fused.len(), 2);
    assert!(fused.iter().any(|item| item.concept.concept_id == 300));
}

fn result(rank: usize, concept_id: i64, concept_name: &str, method: &str) -> SearchResult {
    SearchResult {
        rank,
        concept: ConceptSummary {
            concept_id,
            concept_name: concept_name.to_string(),
            domain_id: "Drug".to_string(),
            vocabulary_id: "RxNorm".to_string(),
            concept_class_id: "Clinical Drug".to_string(),
            standard_concept: "S".to_string(),
            concept_code: concept_id.to_string(),
        },
        scores: serde_json::json!({}),
        component_ranks: serde_json::json!({}),
        method: method.to_string(),
    }
}
