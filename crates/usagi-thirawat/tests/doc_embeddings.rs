use std::fs;

use usagi_contracts::catalog::ConceptSummary;
use usagi_thirawat::artifact::{
    validate_thirawat_doc_embedding_artifact, ThirawatDocEmbeddingArtifactPaths,
};
use usagi_thirawat::doc_embeddings::{
    write_thirawat_doc_embedding_artifact, ThirawatDocEmbeddingBuildOptions,
    ThirawatDocEmbeddingDocument,
};

#[test]
fn writes_manifest_backed_doc_embedding_artifact_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let doc_dir = dir.path().join("doc_embeddings");

    let summary = write_thirawat_doc_embedding_artifact(
        ThirawatDocEmbeddingBuildOptions {
            doc_embedding_dir: doc_dir.clone(),
            catalog_artifact_id: "catalog-v1".to_string(),
            model_artifact_id: "sidataplus/THIRAWAT-SapBERT".to_string(),
            artifact_id: Some("doc-embeddings-v1".to_string()),
            overwrite: false,
        },
        vec![ThirawatDocEmbeddingDocument {
            concept: drug_concept(40162522, "Tramadol Hydrochloride 50 MG Oral Capsule"),
            token_ids: vec![10, 11],
            token_vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        }],
    )
    .unwrap();

    assert_eq!(summary.doc_embedding_artifact_id, "doc-embeddings-v1");
    assert_eq!(summary.document_count, 1);
    assert_eq!(summary.token_count, 2);
    assert_eq!(summary.dimension, 2);
    for filename in [
        "token_vectors.npy",
        "token_ids.npy",
        "doclens.npy",
        "doc_ids.arrow",
        "documents.json",
        "manifest.json",
    ] {
        assert!(doc_dir.join(filename).exists(), "{filename} missing");
    }
    validate_thirawat_doc_embedding_artifact(ThirawatDocEmbeddingArtifactPaths {
        doc_embedding_dir: doc_dir.clone(),
    })
    .unwrap();

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(doc_dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["artifact_id"], "doc-embeddings-v1");
    assert_eq!(manifest["extra"]["counts"]["tokens"], 2);
    assert_eq!(manifest["extra"]["catalog"]["scope"]["domain_id"], "Drug");
    assert!(manifest["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|file| file["sha256"].as_str().is_some_and(|sha| sha.len() == 64)));

    let docs: serde_json::Value =
        serde_json::from_slice(&fs::read(doc_dir.join("documents.json")).unwrap()).unwrap();
    assert_eq!(docs["documents"][0]["concept"]["concept_id"], 40162522);
}

#[test]
fn doc_embedding_validator_rejects_corrupted_manifest_output() {
    let dir = tempfile::tempdir().unwrap();
    let doc_dir = dir.path().join("doc_embeddings");

    write_thirawat_doc_embedding_artifact(
        ThirawatDocEmbeddingBuildOptions {
            doc_embedding_dir: doc_dir.clone(),
            catalog_artifact_id: "catalog-v1".to_string(),
            model_artifact_id: "sidataplus/THIRAWAT-SapBERT".to_string(),
            artifact_id: Some("doc-embeddings-v1".to_string()),
            overwrite: false,
        },
        vec![ThirawatDocEmbeddingDocument {
            concept: drug_concept(40162522, "Tramadol Hydrochloride 50 MG Oral Capsule"),
            token_ids: vec![10, 11],
            token_vectors: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        }],
    )
    .unwrap();

    fs::write(doc_dir.join("token_vectors.npy"), b"corrupted").unwrap();

    let err = validate_thirawat_doc_embedding_artifact(ThirawatDocEmbeddingArtifactPaths {
        doc_embedding_dir: doc_dir,
    })
    .expect_err("corrupted output should fail checksum validation");

    assert!(err.message().contains("checksum mismatch"));
}

#[test]
fn rejects_non_drug_documents() {
    let dir = tempfile::tempdir().unwrap();
    let err = write_thirawat_doc_embedding_artifact(
        ThirawatDocEmbeddingBuildOptions {
            doc_embedding_dir: dir.path().join("doc_embeddings"),
            catalog_artifact_id: "catalog-v1".to_string(),
            model_artifact_id: "sidataplus/THIRAWAT-SapBERT".to_string(),
            artifact_id: None,
            overwrite: false,
        },
        vec![ThirawatDocEmbeddingDocument {
            concept: ConceptSummary {
                concept_id: 1,
                concept_name: "Not a drug".to_string(),
                domain_id: "Condition".to_string(),
                vocabulary_id: "SNOMED".to_string(),
                concept_class_id: "Clinical Finding".to_string(),
                standard_concept: "S".to_string(),
                concept_code: "x".to_string(),
            },
            token_ids: vec![1],
            token_vectors: vec![vec![1.0, 0.0]],
        }],
    )
    .unwrap_err();

    assert!(err.message().contains("standard Drug concepts"));
}

fn drug_concept(concept_id: i64, concept_name: &str) -> ConceptSummary {
    ConceptSummary {
        concept_id,
        concept_name: concept_name.to_string(),
        domain_id: "Drug".to_string(),
        vocabulary_id: "RxNorm".to_string(),
        concept_class_id: "Clinical Drug".to_string(),
        standard_concept: "S".to_string(),
        concept_code: "859751".to_string(),
    }
}
