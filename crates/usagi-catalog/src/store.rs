use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use usagi_common::error::{Result, UsagiError};
use usagi_contracts::catalog::{Concept, ConceptRelationship, ConceptSummary, RelatedConcept};

#[derive(Debug, Clone)]
pub struct CatalogStore {
    path: PathBuf,
}

impl CatalogStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
        })
    }

    fn connection(&self) -> Result<Connection> {
        Connection::open(&self.path).map_err(db_error)
    }

    pub fn get_concept(&self, concept_id: i64) -> Result<Option<Concept>> {
        let conn = self.connection()?;
        select_concept(&conn, concept_id)
    }

    pub fn batch_concepts(&self, concept_ids: &[i64]) -> Result<(Vec<ConceptSummary>, Vec<i64>)> {
        let mut concepts = Vec::new();
        let mut missing = Vec::new();
        for id in concept_ids {
            match self.get_concept(*id)? {
                Some(concept) => concepts.push(ConceptSummary::from(concept)),
                None => missing.push(*id),
            }
        }
        Ok((concepts, missing))
    }

    pub fn ancestors(
        &self,
        concept_id: i64,
        limit: i64,
        offset: i64,
        min_level: i64,
        max_level: Option<i64>,
    ) -> Result<Vec<RelatedConcept>> {
        let conn = self.connection()?;
        hierarchy_query(HierarchyArgs {
            conn: &conn,
            select_id_col: "ancestor_concept_id",
            filter_id_col: "descendant_concept_id",
            concept_id,
            limit,
            offset,
            min_level,
            max_level,
        })
    }

    pub fn descendants(
        &self,
        concept_id: i64,
        limit: i64,
        offset: i64,
        min_level: i64,
        max_level: Option<i64>,
    ) -> Result<Vec<RelatedConcept>> {
        let conn = self.connection()?;
        hierarchy_query(HierarchyArgs {
            conn: &conn,
            select_id_col: "descendant_concept_id",
            filter_id_col: "ancestor_concept_id",
            concept_id,
            limit,
            offset,
            min_level,
            max_level,
        })
    }

    pub fn relationships(
        &self,
        concept_id: i64,
        relationship_id: Option<&str>,
        direction: &str,
    ) -> Result<Vec<ConceptRelationship>> {
        let conn = self.connection()?;
        let mut out = Vec::new();
        if direction == "outgoing" || direction == "both" {
            out.extend(relationship_direction(
                &conn,
                concept_id,
                relationship_id,
                "outgoing",
                true,
            )?);
        }
        if direction == "incoming" || direction == "both" {
            out.extend(relationship_direction(
                &conn,
                concept_id,
                relationship_id,
                "incoming",
                false,
            )?);
        }
        Ok(out)
    }

    pub fn concept_count(&self) -> Result<i64> {
        let conn = self.connection()?;
        conn.query_row("SELECT COUNT(*) FROM concept", [], |row| row.get(0))
            .map_err(db_error)
    }

    pub fn concepts_for_embedding(&self) -> Result<Vec<ConceptSummary>> {
        let conn = self.connection()?;
        concepts_for_embedding_query(
            &conn,
            "SELECT concept_id, concept_name, domain_id, vocabulary_id,
                    concept_class_id, standard_concept, concept_code
             FROM concept
             ORDER BY concept_id",
        )
    }

    pub fn drug_concepts_for_embedding(&self) -> Result<Vec<ConceptSummary>> {
        let conn = self.connection()?;
        concepts_for_embedding_query(
            &conn,
            "SELECT concept_id, concept_name, domain_id, vocabulary_id,
                    concept_class_id, standard_concept, concept_code
             FROM concept
             WHERE domain_id = 'Drug'
             ORDER BY concept_id",
        )
    }

    pub fn domains(&self) -> Result<Vec<serde_json::Value>> {
        let conn = self.connection()?;
        query_json_rows(
            &conn,
            "SELECT domain_id, domain_name, domain_concept_id FROM domain ORDER BY domain_id",
            &["domain_id", "domain_name", "domain_concept_id"],
        )
    }

    pub fn vocabularies(&self) -> Result<Vec<serde_json::Value>> {
        let conn = self.connection()?;
        query_json_rows(
            &conn,
            "SELECT vocabulary_id, vocabulary_name, vocabulary_reference,
                    vocabulary_version, vocabulary_concept_id
             FROM vocabulary
             ORDER BY vocabulary_id",
            &[
                "vocabulary_id",
                "vocabulary_name",
                "vocabulary_reference",
                "vocabulary_version",
                "vocabulary_concept_id",
            ],
        )
    }

    pub fn concept_classes(&self) -> Result<Vec<serde_json::Value>> {
        let conn = self.connection()?;
        query_json_rows(
            &conn,
            "SELECT concept_class_id, concept_class_name, concept_class_concept_id
             FROM concept_class
             ORDER BY concept_class_id",
            &[
                "concept_class_id",
                "concept_class_name",
                "concept_class_concept_id",
            ],
        )
    }
}

fn concepts_for_embedding_query(conn: &Connection, sql: &str) -> Result<Vec<ConceptSummary>> {
    let mut stmt = conn.prepare(sql).map_err(db_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ConceptSummary {
                concept_id: row.get(0)?,
                concept_name: row.get(1)?,
                domain_id: row.get(2)?,
                vocabulary_id: row.get(3)?,
                concept_class_id: row.get(4)?,
                standard_concept: row.get(5)?,
                concept_code: row.get(6)?,
            })
        })
        .map_err(db_error)?;
    collect_rows(rows)
}

pub fn select_concept(conn: &Connection, concept_id: i64) -> Result<Option<Concept>> {
    conn.query_row(
        "SELECT concept_id, concept_name, domain_id, vocabulary_id, concept_class_id,
                standard_concept, concept_code, valid_start_date, valid_end_date, invalid_reason
         FROM concept
         WHERE concept_id = ?1",
        [concept_id],
        |row| {
            Ok(Concept {
                concept_id: row.get(0)?,
                concept_name: row.get(1)?,
                domain_id: row.get(2)?,
                vocabulary_id: row.get(3)?,
                concept_class_id: row.get(4)?,
                standard_concept: row.get(5)?,
                concept_code: row.get(6)?,
                valid_start_date: row.get(7)?,
                valid_end_date: row.get(8)?,
                invalid_reason: row.get(9)?,
            })
        },
    )
    .optional()
    .map_err(db_error)
}

struct HierarchyArgs<'a> {
    conn: &'a Connection,
    select_id_col: &'a str,
    filter_id_col: &'a str,
    concept_id: i64,
    limit: i64,
    offset: i64,
    min_level: i64,
    max_level: Option<i64>,
}

fn hierarchy_query(args: HierarchyArgs<'_>) -> Result<Vec<RelatedConcept>> {
    let max_clause = if args.max_level.is_some() {
        "AND ca.min_levels_of_separation <= ?5"
    } else {
        "AND (?5 IS NULL OR 1 = 1)"
    };
    let sql = format!(
        "SELECT c.concept_id, c.concept_name, c.domain_id, c.vocabulary_id,
                c.concept_class_id, c.standard_concept, c.concept_code,
                ca.min_levels_of_separation, ca.max_levels_of_separation
         FROM concept_ancestor ca
         JOIN concept c ON c.concept_id = ca.{select_id_col}
         WHERE ca.{filter_id_col} = ?1
           AND ca.min_levels_of_separation >= ?2
           {max_clause}
         ORDER BY ca.min_levels_of_separation, c.concept_name, c.concept_id
         LIMIT ?3 OFFSET ?4",
        select_id_col = args.select_id_col,
        filter_id_col = args.filter_id_col,
    );
    let mut stmt = args.conn.prepare(&sql).map_err(db_error)?;
    let rows = stmt
        .query_map(
            params![
                args.concept_id,
                args.min_level,
                args.limit,
                args.offset,
                args.max_level
            ],
            |row| {
                Ok(RelatedConcept {
                    concept: ConceptSummary {
                        concept_id: row.get(0)?,
                        concept_name: row.get(1)?,
                        domain_id: row.get(2)?,
                        vocabulary_id: row.get(3)?,
                        concept_class_id: row.get(4)?,
                        standard_concept: row.get(5)?,
                        concept_code: row.get(6)?,
                    },
                    min_levels_of_separation: row.get(7)?,
                    max_levels_of_separation: row.get(8)?,
                })
            },
        )
        .map_err(db_error)?;
    collect_rows(rows)
}

fn relationship_direction(
    conn: &Connection,
    concept_id: i64,
    relationship_id: Option<&str>,
    direction: &str,
    outgoing: bool,
) -> Result<Vec<ConceptRelationship>> {
    let (filter_col, target_col) = if outgoing {
        ("concept_id_1", "concept_id_2")
    } else {
        ("concept_id_2", "concept_id_1")
    };
    let sql = format!(
        "SELECT cr.relationship_id, c.concept_id, c.concept_name, c.domain_id,
                c.vocabulary_id, c.concept_class_id, c.standard_concept, c.concept_code
         FROM concept_relationship cr
         JOIN concept c ON c.concept_id = cr.{target_col}
         WHERE cr.{filter_col} = ?1
           AND (?2 IS NULL OR cr.relationship_id = ?2)
         ORDER BY cr.relationship_id, c.concept_name, c.concept_id"
    );
    let mut stmt = conn.prepare(&sql).map_err(db_error)?;
    let rows = stmt
        .query_map(params![concept_id, relationship_id], |row| {
            Ok(ConceptRelationship {
                relationship_id: row.get(0)?,
                direction: direction.to_string(),
                target_concept: ConceptSummary {
                    concept_id: row.get(1)?,
                    concept_name: row.get(2)?,
                    domain_id: row.get(3)?,
                    vocabulary_id: row.get(4)?,
                    concept_class_id: row.get(5)?,
                    standard_concept: row.get(6)?,
                    concept_code: row.get(7)?,
                },
            })
        })
        .map_err(db_error)?;
    collect_rows(rows)
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(db_error)?);
    }
    Ok(out)
}

fn query_json_rows(conn: &Connection, sql: &str, keys: &[&str]) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(sql).map_err(db_error)?;
    let rows = stmt
        .query_map([], |row| {
            let mut object = serde_json::Map::new();
            for (idx, key) in keys.iter().enumerate() {
                let value = row.get_ref(idx)?;
                let json_value = match value {
                    rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                    rusqlite::types::ValueRef::Integer(value) => serde_json::Value::from(value),
                    rusqlite::types::ValueRef::Real(value) => serde_json::Value::from(value),
                    rusqlite::types::ValueRef::Text(value) => {
                        serde_json::Value::String(String::from_utf8_lossy(value).to_string())
                    }
                    rusqlite::types::ValueRef::Blob(_) => serde_json::Value::Null,
                };
                object.insert((*key).to_string(), json_value);
            }
            Ok(serde_json::Value::Object(object))
        })
        .map_err(db_error)?;
    collect_rows(rows)
}

fn db_error(err: rusqlite::Error) -> UsagiError {
    UsagiError::internal(err.to_string())
}
