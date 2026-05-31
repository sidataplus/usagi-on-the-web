use usagi_common::manifest::{sha256_file, ArtifactManifest};
use usagi_contracts::catalog::ConceptSummary;
use usagi_tachiom::build::{build_tachiom_index, DocEmbeddingFixture, TachiomBuildOptions};
use usagi_thirawat::mapper::{TachiomFixtureDocument, TachiomFixtureIndex};

#[test]
fn builds_tachiom_index_from_fixture_doc_embeddings() {
    let dir = tempfile::tempdir().unwrap();
    let doc_dir = dir.path().join("doc_embeddings");
    let index_dir = dir.path().join("tachiom");
    std::fs::create_dir_all(&doc_dir).unwrap();

    let fixture = DocEmbeddingFixture {
        documents: vec![TachiomFixtureDocument {
            concept: ConceptSummary {
                concept_id: 1,
                concept_name: "Aspirin 81 MG Oral Tablet".to_string(),
                domain_id: "Drug".to_string(),
                vocabulary_id: "RxNorm".to_string(),
                concept_class_id: "Clinical Drug".to_string(),
                standard_concept: "S".to_string(),
                concept_code: "1".to_string(),
            },
            token_vectors: vec![vec![1.0, 0.0]],
        }],
    };
    std::fs::write(
        doc_dir.join("documents.json"),
        serde_json::to_vec_pretty(&fixture).unwrap(),
    )
    .unwrap();
    for filename in [
        "token_vectors.npy",
        "token_ids.npy",
        "doclens.npy",
        "doc_ids.arrow",
        "manifest.json",
    ] {
        std::fs::write(doc_dir.join(filename), b"fixture").unwrap();
    }

    let summary = build_tachiom_index(TachiomBuildOptions {
        doc_embedding_dir: doc_dir,
        index_dir: index_dir.clone(),
        artifact_id: Some("fixture-tachiom-v1".to_string()),
        overwrite: false,
    })
    .unwrap();

    assert_eq!(summary.tachiom_artifact_id, "fixture-tachiom-v1");
    assert_eq!(summary.document_count, 1);
    assert!(index_dir.join("index.bin").exists());
    assert!(index_dir.join("manifest.json").exists());

    let index: TachiomFixtureIndex =
        serde_json::from_slice(&std::fs::read(index_dir.join("index.bin")).unwrap()).unwrap();
    assert_eq!(index.documents[0].concept.concept_id, 1);

    let manifest: ArtifactManifest =
        serde_json::from_slice(&std::fs::read(index_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.artifact_id, "fixture-tachiom-v1");
    assert_eq!(manifest.artifact_kind, "tachiom-index");
    assert_eq!(
        manifest.outputs[0].sha256.as_deref(),
        Some(sha256_file(index_dir.join("index.bin")).unwrap().as_str())
    );
}

#[test]
fn rejects_non_drug_or_non_standard_documents() {
    let dir = tempfile::tempdir().unwrap();
    let doc_dir = dir.path().join("doc_embeddings");
    let index_dir = dir.path().join("tachiom");
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(
        doc_dir.join("documents.json"),
        serde_json::to_vec_pretty(&DocEmbeddingFixture {
            documents: vec![TachiomFixtureDocument {
                concept: ConceptSummary {
                    concept_id: 1,
                    concept_name: "Bad source concept".to_string(),
                    domain_id: "Drug".to_string(),
                    vocabulary_id: "RxNorm".to_string(),
                    concept_class_id: "Clinical Drug".to_string(),
                    standard_concept: String::new(),
                    concept_code: "1".to_string(),
                },
                token_vectors: vec![vec![1.0, 0.0]],
            }],
        })
        .unwrap(),
    )
    .unwrap();
    for filename in [
        "token_vectors.npy",
        "token_ids.npy",
        "doclens.npy",
        "doc_ids.arrow",
        "manifest.json",
    ] {
        std::fs::write(doc_dir.join(filename), b"fixture").unwrap();
    }

    let err = build_tachiom_index(TachiomBuildOptions {
        doc_embedding_dir: doc_dir,
        index_dir,
        artifact_id: None,
        overwrite: false,
    })
    .unwrap_err();

    assert_eq!(err.code().as_str(), "INCOMPATIBLE_ARTIFACT");
}
