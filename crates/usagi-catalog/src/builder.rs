use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use csv::StringRecord;
use rusqlite::{params, Connection};
use serde::Serialize;
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_common::manifest::{sha256_file, ArtifactManifest, ManifestFile};

#[derive(Debug, Clone)]
pub struct BuildCatalogOptions {
    pub athena_dir: PathBuf,
    pub output_dir: PathBuf,
    pub vocabulary_version: Option<String>,
    pub artifact_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildCatalogSummary {
    pub catalog_artifact_id: String,
    pub concept_count: i64,
    pub manifest_path: PathBuf,
}

pub fn build_catalog_from_athena(options: BuildCatalogOptions) -> Result<BuildCatalogSummary> {
    fs::create_dir_all(&options.output_dir)?;
    let db_path = options.output_dir.join("catalog.sqlite");
    let manifest_path = options.output_dir.join("manifest.json");
    if db_path.exists() {
        fs::remove_file(&db_path)?;
    }

    let mut conn = Connection::open(&db_path).map_err(db_error)?;
    create_schema(&conn)?;
    let standard_ids = load_concepts(&mut conn, &options.athena_dir)?;
    load_synonyms(&mut conn, &options.athena_dir, &standard_ids)?;
    load_relationships(&mut conn, &options.athena_dir, &standard_ids)?;
    load_ancestors(&mut conn, &options.athena_dir, &standard_ids)?;
    load_reference_tables(&mut conn, &options.athena_dir)?;
    create_indexes(&conn)?;

    let concept_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM concept", [], |row| row.get(0))
        .map_err(db_error)?;
    if concept_count == 0 {
        return Err(UsagiError::new(
            ErrorCode::CatalogNotReady,
            "no valid standard concepts loaded",
        ));
    }

    let vocabulary_version = options
        .vocabulary_version
        .unwrap_or_else(|| "unknown".to_string());
    conn.execute(
        "INSERT INTO catalog_metadata(key, value) VALUES
         ('status', 'ready'),
         ('vocabulary_version', ?1),
         ('scope.standard_concept', 'S'),
         ('scope.invalid_reason', '')",
        [vocabulary_version.as_str()],
    )
    .map_err(db_error)?;

    let artifact_id = options.artifact_id.unwrap_or_else(|| {
        format!(
            "athena-{}-standard-v1",
            vocabulary_version.replace(|c: char| !c.is_ascii_alphanumeric(), "")
        )
    });
    let catalog_sha = sha256_file(&db_path)?;
    let manifest = ArtifactManifest {
        artifact_id: artifact_id.clone(),
        artifact_kind: "catalog.sqlite".to_string(),
        schema_version: "usagi-catalog-v1".to_string(),
        api_version: "0.1.0".to_string(),
        created_at: Utc::now().to_rfc3339(),
        inputs: vec![ManifestFile {
            path: options.athena_dir.display().to_string(),
            sha256: None,
            content_type: None,
        }],
        outputs: vec![ManifestFile {
            path: "catalog.sqlite".to_string(),
            sha256: Some(catalog_sha),
            content_type: Some("application/vnd.sqlite3".to_string()),
        }],
        extra: Some(serde_json::json!({
            "vocabulary": {
                "source": "ATHENA",
                "version": vocabulary_version,
                "scope": {
                    "standard_concept": "S",
                    "invalid_reason": null
                }
            },
            "counts": {
                "concepts": concept_count
            }
        })),
    };
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    Ok(BuildCatalogSummary {
        catalog_artifact_id: artifact_id,
        concept_count,
        manifest_path,
    })
}

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE concept (
          concept_id INTEGER PRIMARY KEY,
          concept_name TEXT NOT NULL,
          domain_id TEXT NOT NULL,
          vocabulary_id TEXT NOT NULL,
          concept_class_id TEXT NOT NULL,
          standard_concept TEXT NOT NULL,
          concept_code TEXT NOT NULL,
          valid_start_date TEXT,
          valid_end_date TEXT,
          invalid_reason TEXT
        );

        CREATE TABLE concept_synonym (
          concept_id INTEGER NOT NULL,
          concept_synonym_name TEXT NOT NULL,
          language_concept_id INTEGER
        );

        CREATE TABLE concept_relationship (
          concept_id_1 INTEGER NOT NULL,
          concept_id_2 INTEGER NOT NULL,
          relationship_id TEXT NOT NULL,
          valid_start_date TEXT,
          valid_end_date TEXT,
          invalid_reason TEXT,
          PRIMARY KEY (concept_id_1, concept_id_2, relationship_id)
        );

        CREATE TABLE concept_ancestor (
          ancestor_concept_id INTEGER NOT NULL,
          descendant_concept_id INTEGER NOT NULL,
          min_levels_of_separation INTEGER,
          max_levels_of_separation INTEGER,
          PRIMARY KEY (ancestor_concept_id, descendant_concept_id)
        );

        CREATE TABLE vocabulary (
          vocabulary_id TEXT PRIMARY KEY,
          vocabulary_name TEXT,
          vocabulary_reference TEXT,
          vocabulary_version TEXT,
          vocabulary_concept_id INTEGER
        );

        CREATE TABLE domain (
          domain_id TEXT PRIMARY KEY,
          domain_name TEXT,
          domain_concept_id INTEGER
        );

        CREATE TABLE concept_class (
          concept_class_id TEXT PRIMARY KEY,
          concept_class_name TEXT,
          concept_class_concept_id INTEGER
        );

        CREATE TABLE relationship (
          relationship_id TEXT PRIMARY KEY,
          relationship_name TEXT,
          is_hierarchical TEXT,
          defines_ancestry TEXT,
          reverse_relationship_id TEXT,
          relationship_concept_id INTEGER
        );

        CREATE TABLE catalog_metadata (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        ",
    )
    .map_err(db_error)
}

fn create_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE INDEX idx_concept_name ON concept(concept_name);
        CREATE INDEX idx_concept_domain_vocab_class ON concept(domain_id, vocabulary_id, concept_class_id);
        CREATE INDEX idx_concept_code_vocab ON concept(vocabulary_id, concept_code);
        CREATE INDEX idx_synonym_concept ON concept_synonym(concept_id);
        CREATE INDEX idx_synonym_name ON concept_synonym(concept_synonym_name);
        CREATE INDEX idx_rel_1 ON concept_relationship(concept_id_1, relationship_id);
        CREATE INDEX idx_rel_2 ON concept_relationship(concept_id_2, relationship_id);
        CREATE INDEX idx_ancestor_desc ON concept_ancestor(descendant_concept_id);
        CREATE INDEX idx_ancestor_anc ON concept_ancestor(ancestor_concept_id);
        ",
    )
    .map_err(db_error)
}

fn load_concepts(conn: &mut Connection, athena_dir: &Path) -> Result<HashSet<i64>> {
    let tx = conn.transaction().map_err(db_error)?;
    let mut stmt = tx
        .prepare(
            "INSERT INTO concept (
                concept_id, concept_name, domain_id, vocabulary_id, concept_class_id,
                standard_concept, concept_code, valid_start_date, valid_end_date, invalid_reason
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
        )
        .map_err(db_error)?;
    let mut ids = HashSet::new();
    for record in read_tsv(athena_dir.join("CONCEPT.csv"))? {
        let concept_id = parse_i64(&record, 0)?;
        let standard = field(&record, 5)?;
        let invalid = empty_to_none(field(&record, 9)?);
        if standard == "S" && invalid.is_none() {
            stmt.execute(params![
                concept_id,
                field(&record, 1)?,
                field(&record, 2)?,
                field(&record, 3)?,
                field(&record, 4)?,
                standard,
                field(&record, 6)?,
                empty_to_none(field(&record, 7)?),
                empty_to_none(field(&record, 8)?),
            ])
            .map_err(db_error)?;
            ids.insert(concept_id);
        }
    }
    drop(stmt);
    tx.commit().map_err(db_error)?;
    Ok(ids)
}

fn load_synonyms(
    conn: &mut Connection,
    athena_dir: &Path,
    standard_ids: &HashSet<i64>,
) -> Result<()> {
    let tx = conn.transaction().map_err(db_error)?;
    let mut stmt = tx
        .prepare(
            "INSERT INTO concept_synonym(concept_id, concept_synonym_name, language_concept_id)
             VALUES (?1, ?2, ?3)",
        )
        .map_err(db_error)?;
    for record in read_tsv(athena_dir.join("CONCEPT_SYNONYM.csv"))? {
        let concept_id = parse_i64(&record, 0)?;
        if standard_ids.contains(&concept_id) {
            stmt.execute(params![
                concept_id,
                field(&record, 1)?,
                parse_optional_i64(&record, 2)?
            ])
            .map_err(db_error)?;
        }
    }
    drop(stmt);
    tx.commit().map_err(db_error)
}

fn load_relationships(
    conn: &mut Connection,
    athena_dir: &Path,
    standard_ids: &HashSet<i64>,
) -> Result<()> {
    let tx = conn.transaction().map_err(db_error)?;
    let mut stmt = tx
        .prepare(
            "INSERT OR IGNORE INTO concept_relationship(
                concept_id_1, concept_id_2, relationship_id, valid_start_date, valid_end_date, invalid_reason
             ) VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
        )
        .map_err(db_error)?;
    for record in read_tsv(athena_dir.join("CONCEPT_RELATIONSHIP.csv"))? {
        let concept_id_1 = parse_i64(&record, 0)?;
        let concept_id_2 = parse_i64(&record, 1)?;
        let invalid = empty_to_none(field(&record, 5)?);
        if invalid.is_none()
            && standard_ids.contains(&concept_id_1)
            && standard_ids.contains(&concept_id_2)
        {
            stmt.execute(params![
                concept_id_1,
                concept_id_2,
                field(&record, 2)?,
                empty_to_none(field(&record, 3)?),
                empty_to_none(field(&record, 4)?),
            ])
            .map_err(db_error)?;
        }
    }
    drop(stmt);
    tx.commit().map_err(db_error)
}

fn load_ancestors(
    conn: &mut Connection,
    athena_dir: &Path,
    standard_ids: &HashSet<i64>,
) -> Result<()> {
    let tx = conn.transaction().map_err(db_error)?;
    let mut stmt = tx
        .prepare(
            "INSERT OR IGNORE INTO concept_ancestor(
                ancestor_concept_id, descendant_concept_id, min_levels_of_separation, max_levels_of_separation
             ) VALUES (?1, ?2, ?3, ?4)",
        )
        .map_err(db_error)?;
    for record in read_tsv(athena_dir.join("CONCEPT_ANCESTOR.csv"))? {
        let ancestor = parse_i64(&record, 0)?;
        let descendant = parse_i64(&record, 1)?;
        if standard_ids.contains(&ancestor) && standard_ids.contains(&descendant) {
            stmt.execute(params![
                ancestor,
                descendant,
                parse_optional_i64(&record, 2)?,
                parse_optional_i64(&record, 3)?,
            ])
            .map_err(db_error)?;
        }
    }
    drop(stmt);
    tx.commit().map_err(db_error)
}

fn load_reference_tables(conn: &mut Connection, athena_dir: &Path) -> Result<()> {
    load_table(
        conn,
        athena_dir.join("VOCABULARY.csv"),
        "INSERT OR IGNORE INTO vocabulary VALUES (?1, ?2, ?3, ?4, ?5)",
        5,
    )?;
    load_table(
        conn,
        athena_dir.join("DOMAIN.csv"),
        "INSERT OR IGNORE INTO domain VALUES (?1, ?2, ?3)",
        3,
    )?;
    load_table(
        conn,
        athena_dir.join("CONCEPT_CLASS.csv"),
        "INSERT OR IGNORE INTO concept_class VALUES (?1, ?2, ?3)",
        3,
    )?;
    load_table(
        conn,
        athena_dir.join("RELATIONSHIP.csv"),
        "INSERT OR IGNORE INTO relationship VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        6,
    )
}

fn load_table(conn: &mut Connection, path: PathBuf, sql: &str, columns: usize) -> Result<()> {
    let tx = conn.transaction().map_err(db_error)?;
    let mut stmt = tx.prepare(sql).map_err(db_error)?;
    for record in read_tsv(path)? {
        let values: Vec<Option<String>> = (0..columns)
            .map(|idx| field(&record, idx).map(empty_to_none))
            .collect::<Result<_>>()?;
        stmt.execute(rusqlite::params_from_iter(values.iter()))
            .map_err(db_error)?;
    }
    drop(stmt);
    tx.commit().map_err(db_error)
}

fn read_tsv(path: PathBuf) -> Result<Vec<StringRecord>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(&path)
        .map_err(|err| {
            UsagiError::bad_request(format!("failed to read {}: {err}", path.display()))
        })?;
    reader
        .records()
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|err| UsagiError::bad_request(err.to_string()))
}

fn field(record: &StringRecord, index: usize) -> Result<&str> {
    record
        .get(index)
        .ok_or_else(|| UsagiError::bad_request(format!("missing column {index}")))
}

fn empty_to_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_i64(record: &StringRecord, index: usize) -> Result<i64> {
    field(record, index)?
        .parse::<i64>()
        .map_err(|err| UsagiError::bad_request(format!("invalid integer column {index}: {err}")))
}

fn parse_optional_i64(record: &StringRecord, index: usize) -> Result<Option<i64>> {
    match empty_to_none(field(record, index)?) {
        Some(value) => value.parse::<i64>().map(Some).map_err(|err| {
            UsagiError::bad_request(format!("invalid integer column {index}: {err}"))
        }),
        None => Ok(None),
    }
}

fn db_error(err: rusqlite::Error) -> UsagiError {
    UsagiError::internal(err.to_string())
}
