use std::fs;

use usagi_catalog::builder::{build_catalog_from_athena, BuildCatalogOptions};
use usagi_catalog::store::CatalogStore;

#[test]
fn catalog_builder_loads_only_valid_standard_concepts() {
    let dir = tempfile::tempdir().unwrap();
    let athena = dir.path().join("athena-mini");
    fs::create_dir(&athena).unwrap();
    write_athena_fixture(&athena);

    let out = dir.path().join("catalog");
    let summary = build_catalog_from_athena(BuildCatalogOptions {
        athena_dir: athena.clone(),
        output_dir: out.clone(),
        vocabulary_version: Some("mini-v1".to_string()),
        artifact_id: Some("athena-mini-standard-v1".to_string()),
    })
    .unwrap();

    assert_eq!(summary.concept_count, 3);
    assert!(out.join("catalog.sqlite").exists());
    assert!(out.join("manifest.json").exists());

    let store = CatalogStore::open(out.join("catalog.sqlite")).unwrap();
    assert!(store.get_concept(100).unwrap().is_some());
    assert!(store.get_concept(200).unwrap().is_some());
    assert!(
        store.get_concept(300).unwrap().is_none(),
        "non-standard source concept leaked"
    );
    assert!(
        store.get_concept(400).unwrap().is_none(),
        "invalid standard concept leaked"
    );
    let concepts_for_embedding = store.concepts_for_embedding().unwrap();
    assert_eq!(
        concepts_for_embedding
            .iter()
            .map(|concept| concept.concept_id)
            .collect::<Vec<_>>(),
        vec![100, 200, 500]
    );
    assert!(concepts_for_embedding
        .iter()
        .all(|concept| concept.standard_concept == "S"));
    let drug_concepts_for_embedding = store.drug_concepts_for_embedding().unwrap();
    assert_eq!(
        drug_concepts_for_embedding
            .iter()
            .map(|concept| concept.concept_id)
            .collect::<Vec<_>>(),
        vec![100, 200]
    );
}

#[test]
fn relationships_are_kept_only_when_both_concepts_are_standard() {
    let dir = tempfile::tempdir().unwrap();
    let athena = dir.path().join("athena-mini");
    fs::create_dir(&athena).unwrap();
    write_athena_fixture(&athena);

    let out = dir.path().join("catalog");
    build_catalog_from_athena(BuildCatalogOptions {
        athena_dir: athena,
        output_dir: out.clone(),
        vocabulary_version: None,
        artifact_id: None,
    })
    .unwrap();

    let store = CatalogStore::open(out.join("catalog.sqlite")).unwrap();
    let relationships = store.relationships(100, None, "both").unwrap();
    assert_eq!(relationships.len(), 1);
    assert_eq!(relationships[0].target_concept.concept_id, 200);
}

fn write_athena_fixture(path: &std::path::Path) {
    fs::write(
        path.join("CONCEPT.csv"),
        "concept_id\tconcept_name\tdomain_id\tvocabulary_id\tconcept_class_id\tstandard_concept\tconcept_code\tvalid_start_date\tvalid_end_date\tinvalid_reason\n\
100\tMetformin 500 MG Oral Tablet\tDrug\tRxNorm\tClinical Drug\tS\t860975\t20000101\t20991231\t\n\
200\tMetformin\tDrug\tRxNorm\tIngredient\tS\t6809\t20000101\t20991231\t\n\
300\tMETFORMIN HCL TAB\tDrug\tNDC\t11-digit NDC\t\t0001\t20000101\t20991231\t\n\
400\tOld Drug\tDrug\tRxNorm\tClinical Drug\tS\told\t20000101\t20991231\tD\n\
500\tHypertension\tCondition\tSNOMED\tClinical Finding\tS\t38341003\t20000101\t20991231\t\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_SYNONYM.csv"),
        "concept_id\tconcept_synonym_name\tlanguage_concept_id\n100\tmetformin tablet\t4180186\n300\tnon standard synonym\t4180186\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_RELATIONSHIP.csv"),
        "concept_id_1\tconcept_id_2\trelationship_id\tvalid_start_date\tvalid_end_date\tinvalid_reason\n100\t200\tHas ingredient\t20000101\t20991231\t\n300\t100\tMaps to\t20000101\t20991231\t\n",
    )
    .unwrap();
    fs::write(
        path.join("CONCEPT_ANCESTOR.csv"),
        "ancestor_concept_id\tdescendant_concept_id\tmin_levels_of_separation\tmax_levels_of_separation\n200\t100\t1\t1\n300\t100\t1\t1\n",
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
