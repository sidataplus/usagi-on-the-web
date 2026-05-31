use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::Connection;
use serde::Serialize;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, TantivyDocument, Value, FAST, STORED, STRING, TEXT};
use tantivy::{doc, Index};
use usagi_common::error::{Result, UsagiError};
use usagi_common::manifest::{ArtifactManifest, ManifestFile};
use usagi_contracts::catalog::{ConceptSummary, Provenance};
use usagi_contracts::search::SearchResult;

#[derive(Debug, Clone)]
pub struct TantivyBuildOptions {
    pub catalog_db_path: PathBuf,
    pub index_dir: PathBuf,
    pub catalog_artifact_id: String,
    pub artifact_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TantivyBuildSummary {
    pub tantivy_artifact_id: String,
    pub document_count: usize,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub domain_id: Vec<String>,
    pub vocabulary_id: Vec<String>,
    pub concept_class_id: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TantivySearchOptions {
    pub index_dir: PathBuf,
    pub q: String,
    pub limit: usize,
    pub filters: SearchFilters,
}

#[derive(Clone, Copy)]
struct SearchFields {
    concept_id: Field,
    concept_name: Field,
    concept_code: Field,
    domain_id: Field,
    vocabulary_id: Field,
    concept_class_id: Field,
    standard_concept: Field,
    search_blob: Field,
}

pub fn build_tantivy_index(options: TantivyBuildOptions) -> Result<TantivyBuildSummary> {
    if options.index_dir.exists() {
        fs::remove_dir_all(&options.index_dir)?;
    }
    fs::create_dir_all(&options.index_dir)?;

    let (schema, fields) = search_schema();
    let index = Index::create_in_dir(&options.index_dir, schema.clone()).map_err(search_error)?;
    let mut writer = index.writer(50_000_000).map_err(search_error)?;
    let concepts = read_indexable_concepts(&options.catalog_db_path)?;
    let document_count = concepts.len();

    for concept in concepts {
        writer
            .add_document(doc!(
                fields.concept_id => concept.concept_id as u64,
                fields.concept_name => concept.concept_name,
                fields.concept_code => concept.concept_code,
                fields.domain_id => concept.domain_id,
                fields.vocabulary_id => concept.vocabulary_id,
                fields.concept_class_id => concept.concept_class_id,
                fields.standard_concept => concept.standard_concept,
                fields.search_blob => concept.search_blob,
            ))
            .map_err(search_error)?;
    }
    writer.commit().map_err(search_error)?;

    let artifact_id = options.artifact_id.unwrap_or_else(|| {
        format!(
            "{}-tantivy-v1",
            options.catalog_artifact_id.replace("-standard-v1", "")
        )
    });
    let manifest_path = options.index_dir.join("manifest.json");
    let manifest = ArtifactManifest {
        artifact_id: artifact_id.clone(),
        artifact_kind: "tantivy-index".to_string(),
        schema_version: "usagi-tantivy-v1".to_string(),
        api_version: "0.1.0".to_string(),
        created_at: Utc::now().to_rfc3339(),
        inputs: vec![ManifestFile {
            path: options.catalog_db_path.display().to_string(),
            sha256: None,
            content_type: Some("application/vnd.sqlite3".to_string()),
        }],
        outputs: vec![ManifestFile {
            path: "index/".to_string(),
            sha256: None,
            content_type: None,
        }],
        extra: Some(serde_json::json!({
            "catalog": {
                "artifact_id": options.catalog_artifact_id
            },
            "index": {
                "document_count": document_count,
                "fields": [
                    "concept_id",
                    "concept_name",
                    "concept_code",
                    "domain_id",
                    "vocabulary_id",
                    "concept_class_id",
                    "standard_concept",
                    "search_blob"
                ]
            }
        })),
    };
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    Ok(TantivyBuildSummary {
        tantivy_artifact_id: artifact_id,
        document_count,
        manifest_path,
    })
}

pub fn search_tantivy(options: TantivySearchOptions) -> Result<Vec<SearchResult>> {
    let index = Index::open_in_dir(&options.index_dir).map_err(search_error)?;
    let schema = index.schema();
    let fields = fields_from_schema(&schema)?;
    let reader = index.reader().map_err(search_error)?;
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(&index, vec![fields.concept_name, fields.search_blob]);
    let query = parser
        .parse_query(&options.q)
        .map_err(|err| UsagiError::internal(err.to_string()))?;
    let top_docs = searcher
        .search(
            &query,
            &TopDocs::with_limit(options.limit.saturating_mul(5).max(options.limit)),
        )
        .map_err(search_error)?;

    let mut results = Vec::new();
    for (score, address) in top_docs {
        let doc = searcher
            .doc::<TantivyDocument>(address)
            .map_err(search_error)?;
        let concept = concept_from_doc(&doc, fields)?;
        if !matches_filters(&concept, &options.filters) {
            continue;
        }
        let rank = results.len() + 1;
        results.push(SearchResult {
            rank,
            concept,
            scores: serde_json::json!({
                "tantivy": score,
                "sapbert": null,
                "rrf": null
            }),
            component_ranks: serde_json::json!({
                "tantivy": rank
            }),
            method: "lexical_tantivy".to_string(),
        });
        if results.len() >= options.limit {
            break;
        }
    }
    Ok(results)
}

pub fn provenance(catalog_artifact_id: String, tantivy_artifact_id: String) -> Provenance {
    Provenance {
        catalog_artifact_id: Some(catalog_artifact_id),
        model_artifact_id: None,
        index_artifact_id: Some(tantivy_artifact_id),
    }
}

fn search_schema() -> (Schema, SearchFields) {
    let mut builder = Schema::builder();
    let concept_id = builder.add_u64_field("concept_id", FAST | STORED);
    let concept_name = builder.add_text_field("concept_name", TEXT | STORED);
    let concept_code = builder.add_text_field("concept_code", STRING | STORED);
    let domain_id = builder.add_text_field("domain_id", STRING | STORED);
    let vocabulary_id = builder.add_text_field("vocabulary_id", STRING | STORED);
    let concept_class_id = builder.add_text_field("concept_class_id", STRING | STORED);
    let standard_concept = builder.add_text_field("standard_concept", STRING | STORED);
    let search_blob = builder.add_text_field("search_blob", TEXT);
    (
        builder.build(),
        SearchFields {
            concept_id,
            concept_name,
            concept_code,
            domain_id,
            vocabulary_id,
            concept_class_id,
            standard_concept,
            search_blob,
        },
    )
}

fn fields_from_schema(schema: &Schema) -> Result<SearchFields> {
    Ok(SearchFields {
        concept_id: schema
            .get_field("concept_id")
            .map_err(|err| UsagiError::internal(err.to_string()))?,
        concept_name: schema
            .get_field("concept_name")
            .map_err(|err| UsagiError::internal(err.to_string()))?,
        concept_code: schema
            .get_field("concept_code")
            .map_err(|err| UsagiError::internal(err.to_string()))?,
        domain_id: schema
            .get_field("domain_id")
            .map_err(|err| UsagiError::internal(err.to_string()))?,
        vocabulary_id: schema
            .get_field("vocabulary_id")
            .map_err(|err| UsagiError::internal(err.to_string()))?,
        concept_class_id: schema
            .get_field("concept_class_id")
            .map_err(|err| UsagiError::internal(err.to_string()))?,
        standard_concept: schema
            .get_field("standard_concept")
            .map_err(|err| UsagiError::internal(err.to_string()))?,
        search_blob: schema
            .get_field("search_blob")
            .map_err(|err| UsagiError::internal(err.to_string()))?,
    })
}

#[derive(Debug)]
struct IndexableConcept {
    concept_id: i64,
    concept_name: String,
    domain_id: String,
    vocabulary_id: String,
    concept_class_id: String,
    standard_concept: String,
    concept_code: String,
    search_blob: String,
}

fn read_indexable_concepts(catalog_db_path: &Path) -> Result<Vec<IndexableConcept>> {
    let conn = Connection::open(catalog_db_path).map_err(db_error)?;
    let synonyms = read_synonyms(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT concept_id, concept_name, domain_id, vocabulary_id,
                    concept_class_id, standard_concept, concept_code
             FROM concept
             WHERE standard_concept = 'S'
               AND invalid_reason IS NULL
             ORDER BY concept_id",
        )
        .map_err(db_error)?;
    let rows = stmt
        .query_map([], |row| {
            let concept_id: i64 = row.get(0)?;
            let concept_name: String = row.get(1)?;
            let concept_code: String = row.get(6)?;
            let mut parts = vec![concept_name.clone(), concept_code.clone()];
            if let Some(values) = synonyms.get(&concept_id) {
                parts.extend(values.iter().cloned());
            }
            Ok(IndexableConcept {
                concept_id,
                concept_name,
                domain_id: row.get(2)?,
                vocabulary_id: row.get(3)?,
                concept_class_id: row.get(4)?,
                standard_concept: row.get(5)?,
                concept_code,
                search_blob: parts.join(" "),
            })
        })
        .map_err(db_error)?;
    let mut concepts = Vec::new();
    for row in rows {
        concepts.push(row.map_err(db_error)?);
    }
    Ok(concepts)
}

fn read_synonyms(conn: &Connection) -> Result<HashMap<i64, Vec<String>>> {
    let mut stmt = conn
        .prepare(
            "SELECT concept_id, concept_synonym_name
             FROM concept_synonym
             ORDER BY concept_id, concept_synonym_name",
        )
        .map_err(db_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_error)?;
    let mut synonyms: HashMap<i64, Vec<String>> = HashMap::new();
    for row in rows {
        let (concept_id, synonym) = row.map_err(db_error)?;
        synonyms.entry(concept_id).or_default().push(synonym);
    }
    Ok(synonyms)
}

fn concept_from_doc(doc: &TantivyDocument, fields: SearchFields) -> Result<ConceptSummary> {
    Ok(ConceptSummary {
        concept_id: required_u64(doc, fields.concept_id, "concept_id")? as i64,
        concept_name: required_str(doc, fields.concept_name, "concept_name")?,
        domain_id: required_str(doc, fields.domain_id, "domain_id")?,
        vocabulary_id: required_str(doc, fields.vocabulary_id, "vocabulary_id")?,
        concept_class_id: required_str(doc, fields.concept_class_id, "concept_class_id")?,
        standard_concept: required_str(doc, fields.standard_concept, "standard_concept")?,
        concept_code: required_str(doc, fields.concept_code, "concept_code")?,
    })
}

fn required_u64(doc: &TantivyDocument, field: Field, name: &str) -> Result<u64> {
    doc.get_first(field)
        .and_then(|value| value.as_u64())
        .ok_or_else(|| UsagiError::internal(format!("missing stored u64 field {name}")))
}

fn required_str(doc: &TantivyDocument, field: Field, name: &str) -> Result<String> {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| UsagiError::internal(format!("missing stored string field {name}")))
}

fn matches_filters(concept: &ConceptSummary, filters: &SearchFilters) -> bool {
    filter_matches(&filters.domain_id, &concept.domain_id)
        && filter_matches(&filters.vocabulary_id, &concept.vocabulary_id)
        && filter_matches(&filters.concept_class_id, &concept.concept_class_id)
}

fn filter_matches(allowed: &[String], actual: &str) -> bool {
    allowed.is_empty() || allowed.iter().any(|value| value == actual)
}

fn search_error(err: tantivy::TantivyError) -> UsagiError {
    UsagiError::internal(err.to_string())
}

fn db_error(err: rusqlite::Error) -> UsagiError {
    UsagiError::internal(err.to_string())
}
