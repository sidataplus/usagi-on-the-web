use std::fs;

use usagi_catalog::builder::{build_catalog_from_athena, BuildCatalogOptions};
use usagi_search::dense_index::{
    build_sapbert_dense_index, build_sapbert_dense_index_from_precomputed,
    lookup_precomputed_sapbert_query_vector, search_sapbert_dense,
    search_sapbert_dense_from_precomputed_query, DenseDocument, DenseSearchOptions,
    PrecomputedDenseEmbeddings, PrecomputedDenseQueries, PrecomputedDenseQuery,
    SapbertDenseBuildOptions, SapbertPrecomputedBuildOptions,
};

#[test]
fn dense_index_returns_nearest_standard_concept() {
    let dir = tempfile::tempdir().unwrap();
    let athena = dir.path().join("athena-mini");
    fs::create_dir(&athena).unwrap();
    write_athena_fixture(&athena);

    let catalog_dir = dir.path().join("catalog");
    build_catalog_from_athena(BuildCatalogOptions {
        athena_dir: athena,
        output_dir: catalog_dir.clone(),
        vocabulary_version: Some("mini-v1".to_string()),
        artifact_id: Some("athena-mini-standard-v1".to_string()),
    })
    .unwrap();

    let artifact_dir = dir.path().join("sapbert");
    let summary = build_sapbert_dense_index(SapbertDenseBuildOptions {
        artifact_dir: artifact_dir.clone(),
        catalog_artifact_id: "athena-mini-standard-v1".to_string(),
        model_artifact_id: "sapbert-test-v1".to_string(),
        documents: vec![
            DenseDocument {
                concept_id: 100,
                vector: vec![1.0, 0.0, 0.0],
            },
            DenseDocument {
                concept_id: 200,
                vector: vec![0.0, 1.0, 0.0],
            },
        ],
    })
    .unwrap();

    assert_eq!(summary.document_count, 2);
    assert!(artifact_dir.join("sapbert_cls.usearch").exists());
    assert!(artifact_dir.join("concept_ids.arrow").exists());
    assert!(artifact_dir.join("manifest.json").exists());

    let results = search_sapbert_dense(DenseSearchOptions {
        artifact_dir,
        catalog_db_path: catalog_dir.join("catalog.sqlite"),
        query_vector: vec![0.99, 0.01, 0.0],
        limit: 2,
    })
    .unwrap();

    assert_eq!(results[0].concept.concept_id, 100);
    assert_eq!(results[0].method, "sapbert_cls");
    assert!(results[0].scores["sapbert"].as_f64().unwrap() >= 0.0);
}

#[test]
fn dense_index_rejects_empty_vectors() {
    let dir = tempfile::tempdir().unwrap();
    let err = build_sapbert_dense_index(SapbertDenseBuildOptions {
        artifact_dir: dir.path().join("sapbert"),
        catalog_artifact_id: "catalog-v1".to_string(),
        model_artifact_id: "model-v1".to_string(),
        documents: vec![DenseDocument {
            concept_id: 100,
            vector: Vec::new(),
        }],
    })
    .unwrap_err();

    assert!(err.message().contains("dimension"));
}

#[test]
fn dense_index_builds_and_searches_from_precomputed_embedding_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let athena = dir.path().join("athena-mini");
    fs::create_dir(&athena).unwrap();
    write_athena_fixture(&athena);
    let catalog_dir = dir.path().join("catalog");
    build_catalog_from_athena(BuildCatalogOptions {
        athena_dir: athena,
        output_dir: catalog_dir.clone(),
        vocabulary_version: Some("mini-v1".to_string()),
        artifact_id: Some("athena-mini-standard-v1".to_string()),
    })
    .unwrap();

    let docs_path = dir.path().join("sapbert-docs.json");
    fs::write(
        &docs_path,
        serde_json::to_vec_pretty(&PrecomputedDenseEmbeddings {
            documents: vec![
                DenseDocument {
                    concept_id: 100,
                    vector: vec![1.0, 0.0],
                },
                DenseDocument {
                    concept_id: 200,
                    vector: vec![0.0, 1.0],
                },
            ],
        })
        .unwrap(),
    )
    .unwrap();
    let query_path = dir.path().join("sapbert-queries.json");
    fs::write(
        &query_path,
        serde_json::to_vec_pretty(&PrecomputedDenseQueries {
            queries: vec![PrecomputedDenseQuery {
                q: "tramadol 50 mg capsule".to_string(),
                vector: vec![0.99, 0.01],
            }],
        })
        .unwrap(),
    )
    .unwrap();

    let artifact_dir = dir.path().join("sapbert");
    build_sapbert_dense_index_from_precomputed(SapbertPrecomputedBuildOptions {
        artifact_dir: artifact_dir.clone(),
        catalog_artifact_id: "athena-mini-standard-v1".to_string(),
        model_artifact_id: "sapbert-fixture-v1".to_string(),
        embeddings_path: docs_path,
    })
    .unwrap();
    let results = search_sapbert_dense_from_precomputed_query(
        DenseSearchOptions {
            artifact_dir,
            catalog_db_path: catalog_dir.join("catalog.sqlite"),
            query_vector: Vec::new(),
            limit: 2,
        },
        &query_path,
        "tramadol 50 mg capsule",
    )
    .unwrap();

    assert_eq!(results[0].concept.concept_id, 100);
    assert_eq!(results[0].method, "sapbert_cls");
}

#[test]
fn precomputed_query_lookup_returns_none_for_unknown_queries() {
    let dir = tempfile::tempdir().unwrap();
    let query_path = dir.path().join("queries.json");
    fs::write(
        &query_path,
        serde_json::to_vec_pretty(&PrecomputedDenseQueries {
            queries: vec![PrecomputedDenseQuery {
                q: "tramadol 50 mg capsule".to_string(),
                vector: vec![0.99, 0.01],
            }],
        })
        .unwrap(),
    )
    .unwrap();

    let vector = lookup_precomputed_sapbert_query_vector(&query_path, "metformin 500 mg tablet")
        .unwrap();
    assert!(vector.is_none());

    let vector =
        lookup_precomputed_sapbert_query_vector(&query_path, "tramadol 50 mg capsule").unwrap();
    assert_eq!(vector, Some(vec![0.99, 0.01]));
}

fn write_athena_fixture(path: &std::path::Path) {
    fs::write(
        path.join("CONCEPT.csv"),
        "concept_id\tconcept_name\tdomain_id\tvocabulary_id\tconcept_class_id\tstandard_concept\tconcept_code\tvalid_start_date\tvalid_end_date\tinvalid_reason\n\
100\tTramadol Hydrochloride 50 MG Oral Capsule\tDrug\tRxNorm\tClinical Drug\tS\t859751\t20000101\t20991231\t\n\
200\tTramadol\tDrug\tRxNorm\tIngredient\tS\t10689\t20000101\t20991231\t\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_SYNONYM.csv"),
        "concept_id\tconcept_synonym_name\tlanguage_concept_id\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_RELATIONSHIP.csv"),
        "concept_id_1\tconcept_id_2\trelationship_id\tvalid_start_date\tvalid_end_date\tinvalid_reason\n100\t200\tHas ingredient\t20000101\t20991231\t\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_ANCESTOR.csv"),
        "ancestor_concept_id\tdescendant_concept_id\tmin_levels_of_separation\tmax_levels_of_separation\n200\t100\t1\t1\n",
    )
    .unwrap();
    fs::write(
        path.join("VOCABULARY.csv"),
        "vocabulary_id\tvocabulary_name\tvocabulary_reference\tvocabulary_version\tvocabulary_concept_id\nRxNorm\tRxNorm\tref\tmini\t1\n",
    )
    .unwrap();
    fs::write(
        path.join("DOMAIN.csv"),
        "domain_id\tdomain_name\tdomain_concept_id\nDrug\tDrug\t13\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_CLASS.csv"),
        "concept_class_id\tconcept_class_name\tconcept_class_concept_id\nClinical Drug\tClinical Drug\t1\nIngredient\tIngredient\t2\n",
    )
    .unwrap();
    fs::write(
        path.join("RELATIONSHIP.csv"),
        "relationship_id\trelationship_name\tis_hierarchical\tdefines_ancestry\treverse_relationship_id\trelationship_concept_id\nHas ingredient\tHas ingredient\t1\t0\tIngredient of\t1\n",
    )
    .unwrap();
}
