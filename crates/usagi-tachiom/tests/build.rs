use usagi_common::manifest::{sha256_file, ArtifactManifest, ManifestFile};
use usagi_contracts::catalog::ConceptSummary;
use usagi_tachiom::build::{
    build_tachiom_index, DocEmbeddingFixture, TachiomBuildBackend, TachiomBuildOptions,
    TachiomCliBuildOptions,
};
use usagi_thirawat::artifact::{validate_tachiom_artifact, TachiomArtifactPaths};
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
    write_doc_artifact_files(&doc_dir);

    let summary = build_tachiom_index(TachiomBuildOptions {
        doc_embedding_dir: doc_dir,
        index_dir: index_dir.clone(),
        artifact_id: Some("fixture-tachiom-v1".to_string()),
        overwrite: false,
        backend: TachiomBuildBackend::FixtureExact,
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
        manifest.extra.unwrap()["retrieval"]["engine"],
        "fixture-exact-maxsim"
    );
    assert_eq!(
        manifest.outputs[0].sha256.as_deref(),
        Some(sha256_file(index_dir.join("index.bin")).unwrap().as_str())
    );
}

#[test]
fn cli_backend_invokes_tachiom_build_and_records_native_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let doc_dir = dir.path().join("doc_embeddings");
    let index_dir = dir.path().join("tachiom");
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(
        doc_dir.join("documents.json"),
        serde_json::to_vec_pretty(&DocEmbeddingFixture {
            documents: vec![TachiomFixtureDocument {
                concept: concept(1, "Aspirin 81 MG Oral Tablet", "Drug", "S"),
                token_vectors: vec![vec![1.0, 0.0]],
            }],
        })
        .unwrap(),
    )
    .unwrap();
    write_doc_artifact_files(&doc_dir);

    let args_path = dir.path().join("tachiom-build-args.txt");
    let fake_build = write_fake_tachiom_build(dir.path(), &args_path);

    let summary = build_tachiom_index(TachiomBuildOptions {
        doc_embedding_dir: doc_dir,
        index_dir: index_dir.clone(),
        artifact_id: Some("native-tachiom-v1".to_string()),
        overwrite: false,
        backend: TachiomBuildBackend::Cli(TachiomCliBuildOptions {
            build_bin: fake_build,
            total_centroids: Some(128),
            normalize: true,
        }),
    })
    .unwrap();

    assert_eq!(summary.tachiom_artifact_id, "native-tachiom-v1");
    assert_eq!(summary.document_count, 1);
    assert_eq!(
        std::fs::read(index_dir.join("index.bin")).unwrap(),
        b"native-index"
    );

    let args = std::fs::read_to_string(args_path).unwrap();
    assert!(args.contains("-i"));
    assert!(args.contains("token_vectors.npy"));
    assert!(args.contains("--token-ids-file"));
    assert!(args.contains("token_ids.npy"));
    assert!(args.contains("--doclens-file"));
    assert!(args.contains("doclens.npy"));
    assert!(args.contains("-o"));
    assert!(args.contains("index.bin"));
    assert!(args.contains("--total-centroids"));
    assert!(args.contains("128"));
    assert!(args.contains("--normalize"));

    let manifest: ArtifactManifest =
        serde_json::from_slice(&std::fs::read(index_dir.join("manifest.json")).unwrap()).unwrap();
    let extra = manifest.extra.unwrap();
    assert_eq!(extra["retrieval"]["engine"], "tachiom-cli");
    assert_eq!(extra["retrieval"]["input_format"], "tachiom-npy");
    assert_eq!(extra["build_params"]["total_centroids"], 128);
    assert_eq!(extra["build_params"]["normalize"], true);
}

#[test]
fn tachiom_validator_rejects_corrupted_manifest_output() {
    let dir = tempfile::tempdir().unwrap();
    let doc_dir = dir.path().join("doc_embeddings");
    let index_dir = dir.path().join("tachiom");
    std::fs::create_dir_all(&doc_dir).unwrap();
    let fixture = DocEmbeddingFixture {
        documents: vec![TachiomFixtureDocument {
            concept: concept(1, "Aspirin 81 MG Oral Tablet", "Drug", "S"),
            token_vectors: vec![vec![1.0, 0.0]],
        }],
    };
    std::fs::write(
        doc_dir.join("documents.json"),
        serde_json::to_vec_pretty(&fixture).unwrap(),
    )
    .unwrap();
    write_doc_artifact_files(&doc_dir);

    build_tachiom_index(TachiomBuildOptions {
        doc_embedding_dir: doc_dir,
        index_dir: index_dir.clone(),
        artifact_id: Some("fixture-tachiom-v1".to_string()),
        overwrite: false,
        backend: TachiomBuildBackend::FixtureExact,
    })
    .unwrap();

    std::fs::write(index_dir.join("index.bin"), b"corrupted").unwrap();

    let err = validate_tachiom_artifact(TachiomArtifactPaths { index_dir })
        .expect_err("corrupted index should fail checksum validation");

    assert!(err.message().contains("checksum mismatch"));
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
    write_doc_artifact_files(&doc_dir);

    let err = build_tachiom_index(TachiomBuildOptions {
        doc_embedding_dir: doc_dir,
        index_dir,
        artifact_id: None,
        overwrite: false,
        backend: TachiomBuildBackend::FixtureExact,
    })
    .unwrap_err();

    assert_eq!(err.code().as_str(), "INCOMPATIBLE_ARTIFACT");
}

fn write_fake_tachiom_build(
    dir: &std::path::Path,
    args_path: &std::path::Path,
) -> std::path::PathBuf {
    let bin = dir.join("fake-tachiom-build.sh");
    std::fs::write(
        &bin,
        format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > '{}'
output=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    shift
    output="$1"
  fi
  shift || true
done
printf 'native-index' > "$output"
"#,
            args_path.display()
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&bin).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
    }
    std::fs::set_permissions(&bin, permissions).unwrap();
    bin
}

fn write_doc_artifact_files(doc_dir: &std::path::Path) {
    for filename in [
        "token_vectors.npy",
        "token_ids.npy",
        "doclens.npy",
        "doc_ids.arrow",
    ] {
        std::fs::write(doc_dir.join(filename), b"fixture").unwrap();
    }
    let outputs = [
        "token_vectors.npy",
        "token_ids.npy",
        "doclens.npy",
        "doc_ids.arrow",
    ]
    .iter()
    .map(|filename| ManifestFile {
        path: (*filename).to_string(),
        sha256: Some(sha256_file(doc_dir.join(filename)).unwrap()),
        content_type: Some("application/octet-stream".to_string()),
    })
    .collect::<Vec<_>>();
    std::fs::write(
        doc_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "artifact_id": "doc-embeddings-v1",
            "artifact_kind": "thirawat-doc-embeddings",
            "schema_version": "usagi-thirawat-doc-embeddings-v1",
            "api_version": "0.1.0",
            "created_at": "2026-05-31T00:00:00Z",
            "outputs": outputs,
            "extra": {
                "counts": {
                    "documents": 1,
                    "tokens": 1,
                    "dimension": 2
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

fn concept(
    concept_id: i64,
    concept_name: &str,
    domain_id: &str,
    standard_concept: &str,
) -> ConceptSummary {
    ConceptSummary {
        concept_id,
        concept_name: concept_name.to_string(),
        domain_id: domain_id.to_string(),
        vocabulary_id: "RxNorm".to_string(),
        concept_class_id: "Clinical Drug".to_string(),
        standard_concept: standard_concept.to_string(),
        concept_code: concept_id.to_string(),
    }
}
