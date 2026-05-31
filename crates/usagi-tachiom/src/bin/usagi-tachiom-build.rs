use std::path::PathBuf;

use usagi_tachiom::build::{build_tachiom_index, tachiom_backend_from_env, TachiomBuildOptions};

fn main() -> anyhow::Result<()> {
    let doc_embedding_dir = PathBuf::from(
        std::env::var("THIRAWAT_DOC_EMBEDDING_DIR")
            .unwrap_or_else(|_| "data/mapper/thirawat-drug/doc_embeddings".to_string()),
    );
    let index_dir = PathBuf::from(
        std::env::var("TACHIOM_INDEX_DIR")
            .unwrap_or_else(|_| "data/mapper/thirawat-drug/tachiom".to_string()),
    );
    let summary = build_tachiom_index(TachiomBuildOptions {
        doc_embedding_dir,
        index_dir,
        artifact_id: std::env::var("TACHIOM_ARTIFACT_ID").ok(),
        overwrite: true,
        backend: tachiom_backend_from_env()?,
    })?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
