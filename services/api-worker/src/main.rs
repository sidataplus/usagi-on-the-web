use std::path::PathBuf;

use usagi_artifacts::job_results::{write_mapper_batch_results, JobResultArtifactOptions};
use usagi_catalog::builder::{build_catalog_from_athena, BuildCatalogOptions};
use usagi_catalog::store::CatalogStore;
use usagi_common::error::UsagiError;
use usagi_contracts::catalog::CatalogBuildJobRequest;
use usagi_contracts::catalog::Provenance;
use usagi_contracts::jobs::JobKind;
use usagi_contracts::mapper::{
    MapperDrugBatchItemResponse, MapperDrugBatchJobRequest, MapperDrugBatchRequest,
    MapperDrugBatchResponse,
};
use usagi_embed::artifact::{validate_sapbert_model_artifact, SapbertModelArtifactPaths};
use usagi_embed::xlm_roberta::{
    encode_projected_tokens, encode_sapbert_cls, XlmRobertaEncodeOptions,
    XlmRobertaTokenEncodeOptions,
};
use usagi_jobs::store::JobStore;
use usagi_search::dense_index::{
    build_sapbert_dense_index, build_sapbert_dense_index_from_precomputed, DenseDocument,
    SapbertDenseBuildOptions, SapbertPrecomputedBuildOptions,
};
use usagi_search::tantivy_index::{build_tantivy_index, TantivyBuildOptions};
use usagi_tachiom::build::{build_tachiom_index, TachiomBuildOptions};
use usagi_thirawat::artifact::{
    validate_tachiom_artifact, validate_thirawat_doc_embedding_artifact,
    validate_thirawat_model_artifact, TachiomArtifactPaths, ThirawatDocEmbeddingArtifactPaths,
    ThirawatModelArtifactPaths,
};
use usagi_thirawat::doc_embeddings::{
    write_thirawat_doc_embedding_artifact, ThirawatDocEmbeddingBuildOptions,
    ThirawatDocEmbeddingDocument,
};
use usagi_thirawat::mapper::{map_drug_batch_from_precomputed, map_drug_query_with_vectors};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let jobs_path = PathBuf::from(
        std::env::var("JOBS_DB_PATH").unwrap_or_else(|_| "data/jobs/jobs.sqlite".to_string()),
    );
    let queues =
        std::env::var("WORKER_QUEUES").unwrap_or_else(|_| "catalog,index,embed,map".to_string());
    let queue_names: Vec<&str> = queues
        .split(',')
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .collect();
    let store = JobStore::open(jobs_path)?;
    store.migrate()?;
    if let Some(job) = store.claim_next(&queue_names)? {
        println!(
            "claimed job id={} kind={} queue={}",
            job.id, job.kind, job.queue
        );
        if job.kind == JobKind::CatalogBuild {
            set_stage(&store, &job.id, "validating_input", None)?;
            let input = store
                .input_json(&job.id)?
                .ok_or_else(|| anyhow::anyhow!("claimed job {} has no input", job.id))?;
            let payload: CatalogBuildJobRequest = serde_json::from_value(input)?;
            let catalog_db_path = PathBuf::from(
                std::env::var("CATALOG_DB_PATH")
                    .unwrap_or_else(|_| "data/catalog/catalog.sqlite".to_string()),
            );
            let output_dir = catalog_db_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("data/catalog"));
            set_stage(&store, &job.id, "writing_sqlite", None)?;
            match build_catalog_from_athena(BuildCatalogOptions {
                athena_dir: PathBuf::from(payload.athena_dir),
                output_dir,
                vocabulary_version: None,
                artifact_id: Some("local-catalog-standard-v1".to_string()),
            }) {
                Ok(summary) => {
                    set_stage(&store, &job.id, "validating_artifact", None)?;
                    store.finish_success(&job.id, serde_json::to_value(summary)?)?;
                    println!("finished catalog job id={}", job.id);
                }
                Err(err) => {
                    store.finish_failed(
                        &job.id,
                        serde_json::json!({"code": err.code().as_str(), "message": err.message()}),
                    )?;
                    return Err(anyhow::anyhow!(err.to_string()));
                }
            }
        } else if job.kind == JobKind::TantivyBuild {
            set_stage(&store, &job.id, "validating_catalog", None)?;
            let catalog_db_path = PathBuf::from(
                std::env::var("CATALOG_DB_PATH")
                    .unwrap_or_else(|_| "data/catalog/catalog.sqlite".to_string()),
            );
            let tantivy_index_dir = PathBuf::from(
                std::env::var("TANTIVY_INDEX_DIR")
                    .unwrap_or_else(|_| "data/search/tantivy/index".to_string()),
            );
            set_stage(&store, &job.id, "building_index", None)?;
            match build_tantivy_index(TantivyBuildOptions {
                catalog_db_path,
                index_dir: tantivy_index_dir,
                catalog_artifact_id: "local-catalog-standard-v1".to_string(),
                artifact_id: Some("local-tantivy-v1".to_string()),
            }) {
                Ok(summary) => {
                    set_stage(&store, &job.id, "validating_artifact", None)?;
                    store.finish_success(&job.id, serde_json::to_value(summary)?)?;
                    println!("finished tantivy job id={}", job.id);
                }
                Err(err) => {
                    store.finish_failed(
                        &job.id,
                        serde_json::json!({"code": err.code().as_str(), "message": err.message()}),
                    )?;
                    return Err(anyhow::anyhow!(err.to_string()));
                }
            }
        } else if job.kind == JobKind::SapbertBuild {
            if let Ok(embeddings_path) = std::env::var("SAPBERT_PRECOMPUTED_EMBEDDINGS_PATH") {
                set_stage(&store, &job.id, "validating_catalog", None)?;
                let sapbert_index_dir = PathBuf::from(
                    std::env::var("SAPBERT_INDEX_DIR")
                        .unwrap_or_else(|_| "data/search/sapbert".to_string()),
                );
                set_stage(&store, &job.id, "building_usearch_index", None)?;
                match build_sapbert_dense_index_from_precomputed(SapbertPrecomputedBuildOptions {
                    artifact_dir: sapbert_index_dir,
                    catalog_artifact_id: "local-catalog-standard-v1".to_string(),
                    model_artifact_id: "sapbert-precomputed-fixture-v1".to_string(),
                    embeddings_path: PathBuf::from(embeddings_path),
                }) {
                    Ok(summary) => {
                        set_stage(&store, &job.id, "validating_artifact", None)?;
                        store.finish_success(&job.id, serde_json::to_value(summary)?)?;
                        println!("finished sapbert job id={}", job.id);
                    }
                    Err(err) => {
                        store.finish_failed(&job.id, error_json(&err))?;
                    }
                }
                return Ok(());
            }
            set_stage(&store, &job.id, "validating_model", None)?;
            let sapbert_model_dir = PathBuf::from(
                std::env::var("SAPBERT_MODEL_DIR")
                    .unwrap_or_else(|_| "data/models/sapbert".to_string()),
            );
            match validate_sapbert_model_artifact(SapbertModelArtifactPaths {
                model_dir: sapbert_model_dir,
            }) {
                Ok(artifact) => {
                    set_stage(&store, &job.id, "validating_catalog", None)?;
                    let catalog_db_path = PathBuf::from(
                        std::env::var("CATALOG_DB_PATH")
                            .unwrap_or_else(|_| "data/catalog/catalog.sqlite".to_string()),
                    );
                    let concepts = match CatalogStore::open(catalog_db_path)
                        .and_then(|catalog| catalog.concepts_for_embedding())
                    {
                        Ok(concepts) => concepts,
                        Err(err) => {
                            store.finish_failed(&job.id, error_json(&err))?;
                            return Ok(());
                        }
                    };
                    let concept_names: Vec<String> = concepts
                        .iter()
                        .map(|concept| concept.concept_name.clone())
                        .collect();
                    set_stage(
                        &store,
                        &job.id,
                        "embedding_documents",
                        Some(serde_json::json!({"concept_count": concept_names.len()})),
                    )?;
                    let max_length = std::env::var("SAPBERT_MAX_LENGTH")
                        .ok()
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(96);
                    let embeddings = encode_sapbert_cls(
                        XlmRobertaEncodeOptions {
                            model_dir: artifact.model_dir,
                            max_length,
                        },
                        &concept_names,
                    );
                    let embeddings = match embeddings {
                        Ok(embeddings) => embeddings,
                        Err(err) => {
                            store.finish_failed(&job.id, error_json(&err))?;
                            return Ok(());
                        }
                    };
                    let documents = concepts
                        .into_iter()
                        .zip(embeddings)
                        .map(|(concept, embedding)| DenseDocument {
                            concept_id: concept.concept_id,
                            vector: embedding.vector,
                        })
                        .collect();
                    let sapbert_index_dir = PathBuf::from(
                        std::env::var("SAPBERT_INDEX_DIR")
                            .unwrap_or_else(|_| "data/search/sapbert".to_string()),
                    );
                    set_stage(&store, &job.id, "building_usearch_index", None)?;
                    match build_sapbert_dense_index(SapbertDenseBuildOptions {
                        artifact_dir: sapbert_index_dir,
                        catalog_artifact_id: "local-catalog-standard-v1".to_string(),
                        model_artifact_id: "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR"
                            .to_string(),
                        documents,
                    }) {
                        Ok(summary) => {
                            set_stage(&store, &job.id, "validating_artifact", None)?;
                            store.finish_success(&job.id, serde_json::to_value(summary)?)?;
                            println!("finished sapbert job id={}", job.id);
                        }
                        Err(err) => {
                            store.finish_failed(&job.id, error_json(&err))?;
                        }
                    }
                }
                Err(err) => {
                    store.finish_failed(&job.id, error_json(&err))?;
                }
            }
        } else if job.kind == JobKind::ThirawatDocEmbed {
            set_stage(&store, &job.id, "validating_model", None)?;
            let thirawat_model_dir = PathBuf::from(
                std::env::var("THIRAWAT_MODEL_DIR")
                    .unwrap_or_else(|_| "data/models/thirawat-sapbert".to_string()),
            );
            match validate_thirawat_model_artifact(ThirawatModelArtifactPaths {
                model_dir: thirawat_model_dir.clone(),
            }) {
                Ok(()) => {
                    set_stage(&store, &job.id, "validating_catalog", None)?;
                    let catalog_db_path = PathBuf::from(
                        std::env::var("CATALOG_DB_PATH")
                            .unwrap_or_else(|_| "data/catalog/catalog.sqlite".to_string()),
                    );
                    let concepts = match CatalogStore::open(catalog_db_path)
                        .and_then(|catalog| catalog.drug_concepts_for_embedding())
                    {
                        Ok(concepts) => concepts,
                        Err(err) => {
                            store.finish_failed(&job.id, error_json(&err))?;
                            return Ok(());
                        }
                    };
                    let concept_names: Vec<String> = concepts
                        .iter()
                        .map(|concept| concept.concept_name.clone())
                        .collect();
                    set_stage(
                        &store,
                        &job.id,
                        "embedding_documents",
                        Some(serde_json::json!({"concept_count": concept_names.len()})),
                    )?;
                    let embeddings = encode_projected_tokens(
                        XlmRobertaTokenEncodeOptions {
                            model_dir: thirawat_model_dir.clone(),
                            projection_safetensors: thirawat_model_dir
                                .join("colbert_projection.safetensors"),
                            max_length: 96,
                            output_dim: 128,
                        },
                        &concept_names,
                    );
                    let embeddings = match embeddings {
                        Ok(embeddings) => embeddings,
                        Err(err) => {
                            store.finish_failed(&job.id, error_json(&err))?;
                            return Ok(());
                        }
                    };
                    let documents = concepts
                        .into_iter()
                        .zip(embeddings)
                        .map(|(concept, embedding)| ThirawatDocEmbeddingDocument {
                            concept,
                            token_ids: embedding.token_ids,
                            token_vectors: embedding.vectors,
                        })
                        .collect();
                    let artifact_dir = PathBuf::from(
                        std::env::var("THIRAWAT_ARTIFACT_DIR")
                            .unwrap_or_else(|_| "data/mapper/thirawat-drug".to_string()),
                    );
                    set_stage(&store, &job.id, "writing_doc_embeddings", None)?;
                    match write_thirawat_doc_embedding_artifact(
                        ThirawatDocEmbeddingBuildOptions {
                            doc_embedding_dir: artifact_dir.join("doc_embeddings"),
                            catalog_artifact_id: "local-catalog-standard-v1".to_string(),
                            model_artifact_id: "sidataplus/THIRAWAT-SapBERT".to_string(),
                            artifact_id: Some("local-thirawat-drug-doc-embeddings-v1".to_string()),
                            overwrite: true,
                        },
                        documents,
                    ) {
                        Ok(summary) => {
                            set_stage(&store, &job.id, "validating_artifact", None)?;
                            store.finish_success(&job.id, serde_json::to_value(summary)?)?;
                            println!("finished thirawat doc embedding job id={}", job.id);
                        }
                        Err(err) => {
                            store.finish_failed(&job.id, error_json(&err))?;
                        }
                    }
                }
                Err(err) => {
                    store.finish_failed(&job.id, error_json(&err))?;
                }
            }
        } else if job.kind == JobKind::TachiomBuild {
            set_stage(&store, &job.id, "validating_doc_embeddings", None)?;
            let artifact_dir = PathBuf::from(
                std::env::var("THIRAWAT_ARTIFACT_DIR")
                    .unwrap_or_else(|_| "data/mapper/thirawat-drug".to_string()),
            );
            match validate_thirawat_doc_embedding_artifact(ThirawatDocEmbeddingArtifactPaths {
                doc_embedding_dir: artifact_dir.join("doc_embeddings"),
            }) {
                Ok(()) => {
                    let tachiom_index_dir =
                        PathBuf::from(std::env::var("TACHIOM_INDEX_DIR").unwrap_or_else(|_| {
                            artifact_dir.join("tachiom").display().to_string()
                        }));
                    set_stage(&store, &job.id, "building_tachiom_index", None)?;
                    match build_tachiom_index(TachiomBuildOptions {
                        doc_embedding_dir: artifact_dir.join("doc_embeddings"),
                        index_dir: tachiom_index_dir,
                        artifact_id: Some("local-thirawat-drug-tachiom-v1".to_string()),
                        overwrite: true,
                    }) {
                        Ok(summary) => {
                            set_stage(&store, &job.id, "validating_artifact", None)?;
                            store.finish_success(&job.id, serde_json::to_value(summary)?)?;
                            println!("finished tachiom job id={}", job.id);
                        }
                        Err(err) => {
                            store.finish_failed(&job.id, error_json(&err))?;
                        }
                    }
                }
                Err(err) => {
                    store.finish_failed(&job.id, error_json(&err))?;
                }
            }
        } else if job.kind == JobKind::MapperDrugsBatch {
            set_stage(&store, &job.id, "validating_indexes", None)?;
            let input = store
                .input_json(&job.id)?
                .ok_or_else(|| anyhow::anyhow!("claimed job {} has no input", job.id))?;
            let payload: MapperDrugBatchJobRequest = serde_json::from_value(input)?;
            if payload.items.is_empty() {
                set_stage(
                    &store,
                    &job.id,
                    "writing_results",
                    Some(serde_json::json!({"total": 0})),
                )?;
                let results_root = PathBuf::from(
                    std::env::var("JOB_RESULTS_DIR")
                        .unwrap_or_else(|_| "data/jobs/results".to_string()),
                );
                let artifact = write_mapper_batch_results(
                    JobResultArtifactOptions {
                        results_root,
                        job_id: job.id.clone(),
                        artifact_id: None,
                        provenance: None,
                    },
                    std::iter::empty::<serde_json::Value>(),
                )?;
                set_stage(
                    &store,
                    &job.id,
                    "validating_results",
                    Some(serde_json::json!({"processed": 0, "failed": 0})),
                )?;
                store.finish_success_with_artifact(
                    &job.id,
                    serde_json::json!({
                        "result_artifact_id": artifact.artifact_id,
                        "processed": 0,
                        "failed": 0
                    }),
                    &artifact.path,
                )?;
                println!("finished empty mapper batch job id={}", job.id);
                return Ok(());
            }
            let artifact_dir = PathBuf::from(
                std::env::var("THIRAWAT_ARTIFACT_DIR")
                    .unwrap_or_else(|_| "data/mapper/thirawat-drug".to_string()),
            );
            let tachiom_index_dir = PathBuf::from(
                std::env::var("TACHIOM_INDEX_DIR")
                    .unwrap_or_else(|_| artifact_dir.join("tachiom").display().to_string()),
            );
            match validate_tachiom_artifact(TachiomArtifactPaths {
                index_dir: tachiom_index_dir.clone(),
            }) {
                Ok(()) => {
                    set_stage(
                        &store,
                        &job.id,
                        "embedding_queries",
                        Some(serde_json::json!({"total": payload.items.len()})),
                    )?;
                    set_stage(&store, &job.id, "tachiom_retrieval", None)?;
                    set_stage(&store, &job.id, "bimaxsim_reranking", None)?;
                    set_stage(&store, &job.id, "deterministic_tiebreak", None)?;
                    let batch = if let Ok(query_embeddings_path) =
                        std::env::var("THIRAWAT_QUERY_EMBEDDINGS_PATH")
                    {
                        map_drug_batch_from_precomputed(
                            MapperDrugBatchRequest {
                                mode: payload.mode,
                                candidate_top_k: payload.candidate_top_k,
                                rerank_top_n: payload.rerank_top_n,
                                limit: payload.limit,
                                items: payload.items,
                            },
                            PathBuf::from(query_embeddings_path),
                            &tachiom_index_dir,
                        )?
                    } else {
                        let thirawat_model_dir = PathBuf::from(
                            std::env::var("THIRAWAT_MODEL_DIR")
                                .unwrap_or_else(|_| "data/models/thirawat-sapbert".to_string()),
                        );
                        if let Err(err) =
                            validate_thirawat_model_artifact(ThirawatModelArtifactPaths {
                                model_dir: thirawat_model_dir.clone(),
                            })
                        {
                            store.finish_failed(&job.id, error_json(&err))?;
                            return Ok(());
                        }
                        let mode = payload.mode;
                        let mut items = Vec::with_capacity(payload.items.len());
                        for item in payload.items {
                            let item_id = item.id;
                            let response = encode_projected_tokens(
                                XlmRobertaTokenEncodeOptions {
                                    model_dir: thirawat_model_dir.clone(),
                                    projection_safetensors: thirawat_model_dir
                                        .join("colbert_projection.safetensors"),
                                    max_length: 96,
                                    output_dim: 128,
                                },
                                &[item.source_name.as_str()],
                            )
                            .and_then(|embeddings| {
                                let embedding = embeddings.into_iter().next().ok_or_else(|| {
                                    UsagiError::new(
                                        usagi_common::error::ErrorCode::EmbeddingFailed,
                                        "THIRAWAT query encoder produced no vectors",
                                    )
                                })?;
                                map_drug_query_with_vectors(
                                    &item.source_name,
                                    item.source_code.as_deref(),
                                    embedding.vectors,
                                    &tachiom_index_dir,
                                    usagi_thirawat::mapper::MapperRuntimeOptions {
                                        candidate_top_k: payload.candidate_top_k,
                                        rerank_top_n: payload.rerank_top_n,
                                        limit: payload.limit,
                                        ..Default::default()
                                    },
                                )
                            });
                            match response {
                                Ok(response) => items.push(MapperDrugBatchItemResponse {
                                    id: item_id,
                                    candidates: response.candidates,
                                    error: None,
                                }),
                                Err(err) => items.push(MapperDrugBatchItemResponse {
                                    id: item_id,
                                    candidates: Vec::new(),
                                    error: Some(error_json(&err)),
                                }),
                            }
                        }
                        MapperDrugBatchResponse {
                            mode,
                            items,
                            provenance: Provenance {
                                catalog_artifact_id: None,
                                model_artifact_id: Some("sidataplus/THIRAWAT-SapBERT".to_string()),
                                index_artifact_id: Some(
                                    tachiom_index_dir
                                        .join("manifest.json")
                                        .display()
                                        .to_string(),
                                ),
                            },
                        }
                    };
                    for item in &batch.items {
                        if let Some(error) = &item.error {
                            store.record_item_failure(&job.id, &item.id, error.clone())?;
                        }
                    }
                    let results_root = PathBuf::from(
                        std::env::var("JOB_RESULTS_DIR")
                            .unwrap_or_else(|_| "data/jobs/results".to_string()),
                    );
                    let processed = batch.items.len() as i64;
                    set_stage(
                        &store,
                        &job.id,
                        "writing_results",
                        Some(serde_json::json!({"processed": processed})),
                    )?;
                    let provenance = serde_json::to_value(&batch.provenance)?;
                    let artifact = write_mapper_batch_results(
                        JobResultArtifactOptions {
                            results_root,
                            job_id: job.id.clone(),
                            artifact_id: None,
                            provenance: Some(provenance.clone()),
                        },
                        batch.items,
                    )?;
                    let failed = store.get(&job.id)?.map(|job| job.failed).unwrap_or(0);
                    set_stage(
                        &store,
                        &job.id,
                        "validating_results",
                        Some(serde_json::json!({"processed": processed, "failed": failed})),
                    )?;
                    store.finish_itemized_with_artifact(
                        &job.id,
                        processed,
                        serde_json::json!({
                            "result_artifact_id": artifact.artifact_id,
                            "processed": processed,
                            "failed": failed,
                            "provenance": provenance
                        }),
                        &artifact.path,
                    )?;
                    println!("finished mapper batch job id={}", job.id);
                }
                Err(err) => {
                    store.finish_failed(&job.id, error_json(&err))?;
                }
            }
        } else {
            store.finish_failed(
                &job.id,
                serde_json::json!({
                    "code": "BAD_REQUEST",
                    "message": "worker for this job kind is not implemented in this milestone"
                }),
            )?;
        }
    } else {
        println!("no queued jobs for queues={}", queues);
    }
    Ok(())
}

fn error_json(err: &UsagiError) -> serde_json::Value {
    serde_json::json!({
        "code": err.code().as_str(),
        "message": err.message()
    })
}

fn set_stage(
    store: &JobStore,
    job_id: &str,
    stage: &str,
    payload: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    store.set_stage(job_id, stage, payload)?;
    println!("job id={job_id} stage={stage}");
    Ok(())
}
