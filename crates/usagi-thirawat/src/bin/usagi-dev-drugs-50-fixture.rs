use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use usagi_common::error::{Result, UsagiError};
use usagi_contracts::catalog::ConceptSummary;
use usagi_thirawat::doc_embeddings::{
    write_thirawat_doc_embedding_artifact, ThirawatDocEmbeddingBuildOptions,
    ThirawatDocEmbeddingDocument,
};
use usagi_thirawat::mapper::{PrecomputedQueryEmbedding, PrecomputedQueryEmbeddings};

const THIRAWAT_TOKEN_DIMENSION: usize = 128;
const TOKENS_PER_FIXTURE_DOCUMENT: usize = 6;

#[derive(Debug, Deserialize)]
struct StandardDrugConceptRow {
    concept_id: i64,
    concept_name: String,
    domain_id: String,
    vocabulary_id: String,
    concept_class_id: String,
    standard_concept: String,
    concept_code: String,
}

#[derive(Debug, Deserialize)]
struct SourceTermRow {
    source_code: String,
    source_name: String,
    expected_target_concept_id: i64,
}

fn main() -> Result<()> {
    let args = Args::parse()?;
    let concepts = read_standard_concepts(&args.fixture_dir.join("standard_drug_concepts.csv"))?;
    let source_terms = read_source_terms(&args.fixture_dir.join("source_terms.csv"))?;
    write_fixture_artifacts(&args.artifact_dir, concepts, source_terms)
}

#[derive(Debug)]
struct Args {
    fixture_dir: PathBuf,
    artifact_dir: PathBuf,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut fixture_dir = None;
        let mut artifact_dir = None;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--fixture-dir" => fixture_dir = args.next().map(PathBuf::from),
                "--artifact-dir" => artifact_dir = args.next().map(PathBuf::from),
                other => {
                    return Err(UsagiError::bad_request(format!(
                        "unrecognized argument {other}"
                    )))
                }
            }
        }
        Ok(Self {
            fixture_dir: fixture_dir.ok_or_else(|| {
                UsagiError::bad_request("missing required --fixture-dir argument")
            })?,
            artifact_dir: artifact_dir.ok_or_else(|| {
                UsagiError::bad_request("missing required --artifact-dir argument")
            })?,
        })
    }
}

fn read_standard_concepts(path: &Path) -> Result<Vec<ConceptSummary>> {
    let mut reader = csv::Reader::from_path(path).map_err(|err| {
        UsagiError::bad_request(format!("failed to read {}: {err}", path.display()))
    })?;
    let mut concepts = Vec::new();
    for row in reader.deserialize() {
        let row: StandardDrugConceptRow = row.map_err(|err| {
            UsagiError::bad_request(format!(
                "invalid standard concept row in {}: {err}",
                path.display()
            ))
        })?;
        concepts.push(ConceptSummary {
            concept_id: row.concept_id,
            concept_name: row.concept_name,
            domain_id: row.domain_id,
            vocabulary_id: row.vocabulary_id,
            concept_class_id: row.concept_class_id,
            standard_concept: row.standard_concept,
            concept_code: row.concept_code,
        });
    }
    if concepts.is_empty() {
        return Err(UsagiError::bad_request(format!(
            "{} did not contain any standard concepts",
            path.display()
        )));
    }
    Ok(concepts)
}

fn read_source_terms(path: &Path) -> Result<Vec<SourceTermRow>> {
    let mut reader = csv::Reader::from_path(path).map_err(|err| {
        UsagiError::bad_request(format!("failed to read {}: {err}", path.display()))
    })?;
    let mut rows = Vec::new();
    for row in reader.deserialize() {
        rows.push(row.map_err(|err| {
            UsagiError::bad_request(format!(
                "invalid source term row in {}: {err}",
                path.display()
            ))
        })?);
    }
    if rows.is_empty() {
        return Err(UsagiError::bad_request(format!(
            "{} did not contain any source terms",
            path.display()
        )));
    }
    Ok(rows)
}

fn write_fixture_artifacts(
    artifact_dir: &Path,
    concepts: Vec<ConceptSummary>,
    source_terms: Vec<SourceTermRow>,
) -> Result<()> {
    std::fs::create_dir_all(artifact_dir)?;
    let dimension = THIRAWAT_TOKEN_DIMENSION;
    let concept_ids = concepts
        .iter()
        .map(|concept| (concept.concept_id, ()))
        .collect::<BTreeMap<_, _>>();

    let documents = concepts
        .into_iter()
        .map(|concept| {
            let vector = concept_vector(concept.concept_id, dimension);
            ThirawatDocEmbeddingDocument {
                token_ids: (0..TOKENS_PER_FIXTURE_DOCUMENT)
                    .map(|offset| concept.concept_id * 100 + offset as i64)
                    .collect(),
                token_vectors: (0..TOKENS_PER_FIXTURE_DOCUMENT)
                    .map(|_| vector.clone())
                    .collect(),
                concept,
            }
        })
        .collect::<Vec<_>>();

    write_thirawat_doc_embedding_artifact(
        ThirawatDocEmbeddingBuildOptions {
            doc_embedding_dir: artifact_dir.join("doc_embeddings"),
            catalog_artifact_id: "fixture-dev-drugs-50-catalog-v1".to_string(),
            model_artifact_id: "sidataplus/THIRAWAT-SapBERT".to_string(),
            artifact_id: Some("fixture-dev-drugs-50-doc-embeddings-v1".to_string()),
            overwrite: true,
        },
        documents,
    )?;

    let query_embeddings = source_terms
        .into_iter()
        .map(|row| {
            concept_ids
                .get(&row.expected_target_concept_id)
                .ok_or_else(|| {
                    UsagiError::bad_request(format!(
                        "source term {} expects target {} that is missing from standard concepts",
                        row.source_code, row.expected_target_concept_id
                    ))
                })?;
            Ok(PrecomputedQueryEmbedding {
                source_name: row.source_name,
                source_code: Some(row.source_code),
                token_vectors: vec![concept_vector(row.expected_target_concept_id, dimension)],
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let query_dir = artifact_dir.join("query_embeddings");
    std::fs::create_dir_all(&query_dir)?;
    std::fs::write(
        query_dir.join("query_embeddings.json"),
        serde_json::to_vec_pretty(&PrecomputedQueryEmbeddings {
            items: query_embeddings,
        })?,
    )?;
    Ok(())
}

fn concept_vector(concept_id: i64, dimension: usize) -> Vec<f32> {
    let mut state = concept_id as u64 ^ 0x9e37_79b9_7f4a_7c15;
    let mut vector = Vec::with_capacity(dimension);
    for _ in 0..dimension {
        state = splitmix64(state);
        let unit = ((state >> 40) as f32) / ((1_u32 << 24) as f32);
        vector.push(unit.mul_add(2.0, -1.0));
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}
