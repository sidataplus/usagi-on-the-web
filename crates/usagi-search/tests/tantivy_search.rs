use std::fs;

use usagi_catalog::builder::{build_catalog_from_athena, BuildCatalogOptions};
use usagi_search::tantivy_index::{
    build_tantivy_index, search_tantivy, SearchFilters, TantivyBuildOptions, TantivySearchOptions,
};

#[test]
fn tantivy_search_returns_expected_standard_concept() {
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

    let index_dir = dir.path().join("tantivy");
    let summary = build_tantivy_index(TantivyBuildOptions {
        catalog_db_path: catalog_dir.join("catalog.sqlite"),
        index_dir: index_dir.clone(),
        catalog_artifact_id: "athena-mini-standard-v1".to_string(),
        artifact_id: Some("athena-mini-tantivy-v1".to_string()),
    })
    .unwrap();
    assert_eq!(summary.document_count, 2);
    assert!(index_dir.join("manifest.json").exists());

    let results = search_tantivy(TantivySearchOptions {
        index_dir,
        q: "tramadol 50 mg capsule".to_string(),
        limit: 5,
        filters: SearchFilters {
            domain_id: vec!["Drug".to_string()],
            vocabulary_id: vec!["RxNorm".to_string()],
            concept_class_id: vec!["Clinical Drug".to_string()],
        },
    })
    .unwrap();

    assert_eq!(results[0].concept.concept_id, 100);
    assert_eq!(results[0].method, "lexical_tantivy");
}

#[test]
fn tantivy_index_excludes_non_standard_source_terms() {
    let dir = tempfile::tempdir().unwrap();
    let athena = dir.path().join("athena-mini");
    fs::create_dir(&athena).unwrap();
    write_athena_fixture(&athena);

    let catalog_dir = dir.path().join("catalog");
    build_catalog_from_athena(BuildCatalogOptions {
        athena_dir: athena,
        output_dir: catalog_dir.clone(),
        vocabulary_version: None,
        artifact_id: None,
    })
    .unwrap();

    let index_dir = dir.path().join("tantivy");
    build_tantivy_index(TantivyBuildOptions {
        catalog_db_path: catalog_dir.join("catalog.sqlite"),
        index_dir: index_dir.clone(),
        catalog_artifact_id: "local-catalog-standard-v1".to_string(),
        artifact_id: None,
    })
    .unwrap();

    let results = search_tantivy(TantivySearchOptions {
        index_dir,
        q: "LOCAL TRAMADOL SRC".to_string(),
        limit: 10,
        filters: SearchFilters::default(),
    })
    .unwrap();

    assert!(results.iter().all(|item| item.concept.concept_id != 300));
}

fn write_athena_fixture(path: &std::path::Path) {
    fs::write(
        path.join("CONCEPT.csv"),
        "concept_id\tconcept_name\tdomain_id\tvocabulary_id\tconcept_class_id\tstandard_concept\tconcept_code\tvalid_start_date\tvalid_end_date\tinvalid_reason\n\
100\tTramadol Hydrochloride 50 MG Oral Capsule\tDrug\tRxNorm\tClinical Drug\tS\t859751\t20000101\t20991231\t\n\
200\tTramadol\tDrug\tRxNorm\tIngredient\tS\t10689\t20000101\t20991231\t\n\
300\tLOCAL TRAMADOL SRC\tDrug\tLocal\tSource Drug\t\tSRC1\t20000101\t20991231\t\n\
400\tOld Tramadol\tDrug\tRxNorm\tClinical Drug\tS\told\t20000101\t20991231\tD\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_SYNONYM.csv"),
        "concept_id\tconcept_synonym_name\tlanguage_concept_id\n100\ttramadol 50 mg capsule\t4180186\n300\tlocal tramadol source\t4180186\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_RELATIONSHIP.csv"),
        "concept_id_1\tconcept_id_2\trelationship_id\tvalid_start_date\tvalid_end_date\tinvalid_reason\n100\t200\tHas ingredient\t20000101\t20991231\t\n300\t100\tMaps to\t20000101\t20991231\t\n",
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
