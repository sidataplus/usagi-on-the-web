use std::process::Command;

#[test]
fn dev_drugs_fixture_uses_thirawat_token_dimension() {
    let dir = tempfile::tempdir().unwrap();
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/dev-drugs-50");
    let artifact_dir = dir.path().join("thirawat-drug");

    let output = Command::new(env!("CARGO_BIN_EXE_usagi-dev-drugs-50-fixture"))
        .arg("--fixture-dir")
        .arg(&fixture_dir)
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture generator failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifact_dir.join("doc_embeddings/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["extra"]["counts"]["dimension"], 128);
    assert_eq!(manifest["extra"]["counts"]["tokens"], 300);

    let query_embeddings: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifact_dir.join("query_embeddings/query_embeddings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        query_embeddings["items"][0]["token_vectors"][0]
            .as_array()
            .unwrap()
            .len(),
        128
    );

    let token_vectors =
        std::fs::read(artifact_dir.join("doc_embeddings/token_vectors.npy")).unwrap();
    let header_len = token_vectors.len().min(160);
    let header = String::from_utf8_lossy(&token_vectors[..header_len]);
    assert!(
        header.contains("'shape': (300, 128)"),
        "unexpected token vector header {header:?}"
    );
}

#[test]
fn dev_drugs_fixture_supports_more_than_fifty_smoke_targets() {
    let dir = tempfile::tempdir().unwrap();
    let fixture_dir = dir.path().join("fixture");
    let artifact_dir = dir.path().join("thirawat-drug");
    std::fs::create_dir_all(&fixture_dir).unwrap();
    write_synthetic_fixture(&fixture_dir, 150);

    let output = Command::new(env!("CARGO_BIN_EXE_usagi-dev-drugs-50-fixture"))
        .arg("--fixture-dir")
        .arg(&fixture_dir)
        .arg("--artifact-dir")
        .arg(&artifact_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture generator failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifact_dir.join("doc_embeddings/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["extra"]["counts"]["documents"], 150);
    assert_eq!(manifest["extra"]["counts"]["tokens"], 900);
    assert_eq!(manifest["extra"]["counts"]["dimension"], 128);

    let token_vectors =
        std::fs::read(artifact_dir.join("doc_embeddings/token_vectors.npy")).unwrap();
    let header_len = token_vectors.len().min(160);
    let header = String::from_utf8_lossy(&token_vectors[..header_len]);
    assert!(
        header.contains("'shape': (900, 128)"),
        "unexpected token vector header {header:?}"
    );
}

fn write_synthetic_fixture(dir: &std::path::Path, count: usize) {
    let mut concepts =
        "concept_id,concept_name,domain_id,vocabulary_id,concept_class_id,standard_concept,concept_code\n"
            .to_string();
    let mut source_terms = "source_code,source_name,expected_target_concept_id\n".to_string();
    for index in 0..count {
        let concept_id = 1_000_000 + index as i64;
        concepts.push_str(&format!(
            "{concept_id},Synthetic Drug {index},Drug,RxNorm,Clinical Drug,S,SYN{index}\n"
        ));
        source_terms.push_str(&format!("SRC{index},synthetic drug {index},{concept_id}\n"));
    }
    std::fs::write(dir.join("standard_drug_concepts.csv"), concepts).unwrap();
    std::fs::write(dir.join("source_terms.csv"), source_terms).unwrap();
}
