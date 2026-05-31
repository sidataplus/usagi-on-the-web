use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use usagi_common::error::{ErrorCode, Result, UsagiError};
use usagi_contracts::jobs::{JobDto, JobEventDto, JobKind, JobState};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct JobStore {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CreateJob {
    pub kind: JobKind,
    pub queue: String,
    pub idempotency_key: String,
    pub input: Value,
    pub total: i64,
}

impl JobStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self { path })
    }

    fn connection(&self) -> Result<Connection> {
        Connection::open(&self.path).map_err(db_error)
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS jobs (
              id TEXT PRIMARY KEY,
              kind TEXT NOT NULL,
              queue TEXT NOT NULL,
              state TEXT NOT NULL,
              stage TEXT,
              idempotency_key TEXT UNIQUE,
              input_json TEXT NOT NULL,
              result_json TEXT,
              error_json TEXT,
              artifact_path TEXT,
              total INTEGER NOT NULL DEFAULT 0,
              processed INTEGER NOT NULL DEFAULT 0,
              failed INTEGER NOT NULL DEFAULT 0,
              cancel_requested INTEGER NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL,
              started_at TEXT,
              finished_at TEXT,
              updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_jobs_state_queue
              ON jobs(state, queue, created_at);

            CREATE INDEX IF NOT EXISTS idx_jobs_idempotency
              ON jobs(idempotency_key);

            CREATE TABLE IF NOT EXISTS job_events (
              id TEXT PRIMARY KEY,
              job_id TEXT NOT NULL,
              seq INTEGER NOT NULL,
              level TEXT NOT NULL,
              message TEXT NOT NULL,
              payload_json TEXT,
              created_at TEXT NOT NULL,
              FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_job_events_seq
              ON job_events(job_id, seq);

            CREATE TABLE IF NOT EXISTS job_items (
              id TEXT PRIMARY KEY,
              job_id TEXT NOT NULL,
              item_key TEXT NOT NULL,
              state TEXT NOT NULL,
              input_json TEXT,
              result_json TEXT,
              error_json TEXT,
              attempt_count INTEGER NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_job_items_key
              ON job_items(job_id, item_key);

            CREATE INDEX IF NOT EXISTS idx_job_items_state
              ON job_items(job_id, state);
            ",
        )
        .map_err(db_error)
    }

    pub fn create_job(&self, request: CreateJob) -> Result<JobDto> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;

        if let Some(existing) = select_by_idempotency(&tx, &request.idempotency_key)? {
            tx.commit().map_err(db_error)?;
            return Ok(existing);
        }

        let now = now();
        let id = format!("job_{}", Uuid::new_v4().simple());
        tx.execute(
            "INSERT INTO jobs (
                id, kind, queue, state, idempotency_key, input_json,
                total, processed, failed, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8, ?8)",
            params![
                id,
                request.kind.as_str(),
                request.queue,
                JobState::Queued.as_str(),
                request.idempotency_key,
                serde_json::to_string(&request.input)?,
                request.total,
                now,
            ],
        )
        .map_err(db_error)?;
        let job =
            select_job(&tx, &id)?.ok_or_else(|| UsagiError::internal("created job missing"))?;
        tx.commit().map_err(db_error)?;
        Ok(job)
    }

    pub fn claim_next(&self, queues: &[&str]) -> Result<Option<JobDto>> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;

        let job_id = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, queue
                     FROM jobs
                     WHERE state = 'queued'
                     ORDER BY created_at
                     LIMIT 50",
                )
                .map_err(db_error)?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(db_error)?;
            let mut found = None;
            for row in rows {
                let (id, queue) = row.map_err(db_error)?;
                if queues.iter().any(|allowed| *allowed == queue) {
                    found = Some(id);
                    break;
                }
            }
            found
        };

        let Some(id) = job_id else {
            tx.commit().map_err(db_error)?;
            return Ok(None);
        };

        let now = now();
        tx.execute(
            "UPDATE jobs
             SET state = 'running',
                 started_at = COALESCE(started_at, ?2),
                 updated_at = ?2
             WHERE id = ?1 AND state = 'queued'",
            params![id, now],
        )
        .map_err(db_error)?;
        let job = select_job(&tx, &id)?;
        tx.commit().map_err(db_error)?;
        Ok(job)
    }

    pub fn record_item_failure(&self, job_id: &str, item_key: &str, error: Value) -> Result<()> {
        let conn = self.connection()?;
        let now = now();
        conn.execute(
            "INSERT INTO job_items (
                id, job_id, item_key, state, error_json, attempt_count, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'failed', ?4, 1, ?5, ?5)
             ON CONFLICT(job_id, item_key) DO UPDATE SET
                state = 'failed',
                error_json = excluded.error_json,
                attempt_count = job_items.attempt_count + 1,
                updated_at = excluded.updated_at",
            params![
                format!("job_item_{}", Uuid::new_v4().simple()),
                job_id,
                item_key,
                serde_json::to_string(&error)?,
                now,
            ],
        )
        .map_err(db_error)?;
        conn.execute(
            "UPDATE jobs
             SET failed = (SELECT COUNT(*) FROM job_items WHERE job_id = ?1 AND state = 'failed'),
                 updated_at = ?2
             WHERE id = ?1",
            params![job_id, now],
        )
        .map_err(db_error)?;
        Ok(())
    }

    pub fn finish_itemized(&self, job_id: &str, processed: i64) -> Result<JobDto> {
        let conn = self.connection()?;
        let failed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM job_items WHERE job_id = ?1 AND state = 'failed'",
                [job_id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let state = if failed > 0 {
            JobState::SucceededWithErrors
        } else {
            JobState::Succeeded
        };
        let now = now();
        conn.execute(
            "UPDATE jobs
             SET state = ?2, processed = ?3, failed = ?4, finished_at = ?5, updated_at = ?5
             WHERE id = ?1",
            params![job_id, state.as_str(), processed, failed, now],
        )
        .map_err(db_error)?;
        self.get(job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))
    }

    pub fn finish_itemized_with_artifact(
        &self,
        job_id: &str,
        processed: i64,
        result: Value,
        artifact_path: impl AsRef<Path>,
    ) -> Result<JobDto> {
        let conn = self.connection()?;
        let failed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM job_items WHERE job_id = ?1 AND state = 'failed'",
                [job_id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let state = if failed > 0 {
            JobState::SucceededWithErrors
        } else {
            JobState::Succeeded
        };
        let now = now();
        conn.execute(
            "UPDATE jobs
             SET state = ?2,
                 result_json = ?3,
                 artifact_path = ?4,
                 processed = ?5,
                 failed = ?6,
                 finished_at = ?7,
                 updated_at = ?7
             WHERE id = ?1",
            params![
                job_id,
                state.as_str(),
                serde_json::to_string(&result)?,
                artifact_path.as_ref().display().to_string(),
                processed,
                failed,
                now,
            ],
        )
        .map_err(db_error)?;
        self.get(job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))
    }

    pub fn get(&self, job_id: &str) -> Result<Option<JobDto>> {
        let conn = self.connection()?;
        select_job(&conn, job_id)
    }

    pub fn input_json(&self, job_id: &str) -> Result<Option<Value>> {
        let conn = self.connection()?;
        let value = conn
            .query_row(
                "SELECT input_json FROM jobs WHERE id = ?1",
                [job_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(db_error)?;
        value
            .map(|text| serde_json::from_str(&text).map_err(UsagiError::from))
            .transpose()
    }

    pub fn result_json(&self, job_id: &str) -> Result<Option<Value>> {
        let conn = self.connection()?;
        let value = conn
            .query_row(
                "SELECT result_json FROM jobs WHERE id = ?1",
                [job_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(db_error)?
            .flatten();
        value
            .map(|text| serde_json::from_str(&text).map_err(UsagiError::from))
            .transpose()
    }

    pub fn finish_success(&self, job_id: &str, result: Value) -> Result<JobDto> {
        let conn = self.connection()?;
        let now = now();
        conn.execute(
            "UPDATE jobs
             SET state = 'succeeded',
                 result_json = ?2,
                 processed = total,
                 finished_at = ?3,
                 updated_at = ?3
             WHERE id = ?1",
            params![job_id, serde_json::to_string(&result)?, now],
        )
        .map_err(db_error)?;
        self.get(job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))
    }

    pub fn finish_success_with_artifact(
        &self,
        job_id: &str,
        result: Value,
        artifact_path: impl AsRef<Path>,
    ) -> Result<JobDto> {
        let conn = self.connection()?;
        let now = now();
        conn.execute(
            "UPDATE jobs
             SET state = 'succeeded',
                 result_json = ?2,
                 artifact_path = ?3,
                 processed = total,
                 finished_at = ?4,
                 updated_at = ?4
             WHERE id = ?1",
            params![
                job_id,
                serde_json::to_string(&result)?,
                artifact_path.as_ref().display().to_string(),
                now,
            ],
        )
        .map_err(db_error)?;
        self.get(job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))
    }

    pub fn finish_failed(&self, job_id: &str, error: Value) -> Result<JobDto> {
        let conn = self.connection()?;
        let now = now();
        conn.execute(
            "UPDATE jobs
             SET state = 'failed',
                 error_json = ?2,
                 finished_at = ?3,
                 updated_at = ?3
             WHERE id = ?1",
            params![job_id, serde_json::to_string(&error)?, now],
        )
        .map_err(db_error)?;
        self.get(job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))
    }

    pub fn error_json(&self, job_id: &str) -> Result<Option<Value>> {
        let conn = self.connection()?;
        let value = conn
            .query_row(
                "SELECT error_json FROM jobs WHERE id = ?1",
                [job_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(db_error)?
            .flatten();
        value
            .map(|text| serde_json::from_str(&text).map_err(UsagiError::from))
            .transpose()
    }

    pub fn artifact_path(&self, job_id: &str) -> Result<Option<String>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT artifact_path FROM jobs WHERE id = ?1",
            [job_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(db_error)
        .map(Option::flatten)
    }

    pub fn record_event(
        &self,
        job_id: &str,
        level: &str,
        message: &str,
        payload: Option<Value>,
    ) -> Result<JobEventDto> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if select_job(&tx, job_id)?.is_none() {
            return Err(UsagiError::new(ErrorCode::JobNotFound, "job not found"));
        }
        let seq: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM job_events WHERE job_id = ?1",
                [job_id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let id = format!("job_event_{}", Uuid::new_v4().simple());
        let now = now();
        tx.execute(
            "INSERT INTO job_events (
                id, job_id, seq, level, message, payload_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                job_id,
                seq,
                level,
                message,
                payload
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(UsagiError::from)?,
                now,
            ],
        )
        .map_err(db_error)?;
        let event = select_event(&tx, &id)?
            .ok_or_else(|| UsagiError::internal("created job event missing"))?;
        tx.commit().map_err(db_error)?;
        Ok(event)
    }

    pub fn set_stage(&self, job_id: &str, stage: &str, payload: Option<Value>) -> Result<JobDto> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if select_job(&tx, job_id)?.is_none() {
            return Err(UsagiError::new(ErrorCode::JobNotFound, "job not found"));
        }
        let now = now();
        tx.execute(
            "UPDATE jobs
             SET stage = ?2,
                 updated_at = ?3
             WHERE id = ?1",
            params![job_id, stage, now],
        )
        .map_err(db_error)?;

        let seq: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM job_events WHERE job_id = ?1",
                [job_id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let mut event_payload = payload.unwrap_or(Value::Object(Default::default()));
        if let Some(map) = event_payload.as_object_mut() {
            map.insert("stage".to_string(), Value::String(stage.to_string()));
        } else {
            event_payload = serde_json::json!({
                "stage": stage,
                "details": event_payload
            });
        }
        tx.execute(
            "INSERT INTO job_events (
                id, job_id, seq, level, message, payload_json, created_at
             ) VALUES (?1, ?2, ?3, 'info', ?4, ?5, ?6)",
            params![
                format!("job_event_{}", Uuid::new_v4().simple()),
                job_id,
                seq,
                format!("stage:{stage}"),
                serde_json::to_string(&event_payload)?,
                now,
            ],
        )
        .map_err(db_error)?;
        let job = select_job(&tx, job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))?;
        tx.commit().map_err(db_error)?;
        Ok(job)
    }

    pub fn events(&self, job_id: &str) -> Result<Vec<JobEventDto>> {
        let conn = self.connection()?;
        if select_job(&conn, job_id)?.is_none() {
            return Err(UsagiError::new(ErrorCode::JobNotFound, "job not found"));
        }
        let mut stmt = conn
            .prepare(
                "SELECT id, job_id, seq, level, message, payload_json, created_at
                 FROM job_events
                 WHERE job_id = ?1
                 ORDER BY seq",
            )
            .map_err(db_error)?;
        let rows = stmt.query_map([job_id], event_from_row).map_err(db_error)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row.map_err(db_error)?);
        }
        Ok(events)
    }

    pub fn cancel(&self, job_id: &str) -> Result<JobDto> {
        let conn = self.connection()?;
        let existing = self
            .get(job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))?;
        if matches!(
            existing.state,
            JobState::Succeeded | JobState::SucceededWithErrors | JobState::Failed
        ) {
            return Err(UsagiError::new(
                ErrorCode::JobFailed,
                "only queued or running jobs can be cancelled",
            ));
        }
        let now = now();
        conn.execute(
            "UPDATE jobs
             SET state = 'cancelled',
                 cancel_requested = 1,
                 finished_at = COALESCE(finished_at, ?2),
                 updated_at = ?2
             WHERE id = ?1",
            params![job_id, now],
        )
        .map_err(db_error)?;
        self.get(job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))
    }

    pub fn retry(&self, job_id: &str) -> Result<JobDto> {
        let mut conn = self.connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let existing = select_job(&tx, job_id)?
            .ok_or_else(|| UsagiError::new(ErrorCode::JobNotFound, "job not found"))?;
        if !matches!(
            existing.state,
            JobState::Failed | JobState::Cancelled | JobState::SucceededWithErrors
        ) {
            return Err(UsagiError::new(
                ErrorCode::BadRequest,
                "only failed, cancelled, or succeeded_with_errors jobs can be retried",
            ));
        }
        let (idempotency_key, input_json): (String, String) = tx
            .query_row(
                "SELECT idempotency_key, input_json FROM jobs WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(db_error)?;
        let new_id = format!("job_{}", Uuid::new_v4().simple());
        let retry_key = format!("{}::retry::{}", idempotency_key, Uuid::new_v4().simple());
        let now = now();
        tx.execute(
            "INSERT INTO jobs (
                id, kind, queue, state, idempotency_key, input_json,
                total, processed, failed, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6, 0, 0, ?7, ?7)",
            params![
                new_id,
                existing.kind.as_str(),
                existing.queue,
                retry_key,
                input_json,
                existing.total,
                now,
            ],
        )
        .map_err(db_error)?;
        let retried =
            select_job(&tx, &new_id)?.ok_or_else(|| UsagiError::internal("retried job missing"))?;
        tx.commit().map_err(db_error)?;
        Ok(retried)
    }
}

fn select_by_idempotency(conn: &Connection, key: &str) -> Result<Option<JobDto>> {
    let id = conn
        .query_row(
            "SELECT id FROM jobs WHERE idempotency_key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_error)?;
    match id {
        Some(id) => select_job(conn, &id),
        None => Ok(None),
    }
}

fn select_job(conn: &Connection, id: &str) -> Result<Option<JobDto>> {
    conn.query_row(
        "SELECT id, kind, queue, state, stage, processed, total, failed, artifact_path,
                created_at, started_at, finished_at, updated_at
         FROM jobs
         WHERE id = ?1",
        [id],
        |row| {
            let kind_text: String = row.get(1)?;
            let state_text: String = row.get(3)?;
            let kind = JobKind::try_from(kind_text.as_str()).map_err(to_sql_error)?;
            let state = JobState::try_from(state_text.as_str()).map_err(to_sql_error)?;
            Ok(JobDto {
                id: row.get(0)?,
                kind,
                queue: row.get(2)?,
                state,
                stage: row.get(4)?,
                processed: row.get(5)?,
                total: row.get(6)?,
                failed: row.get(7)?,
                artifact_path: row.get(8)?,
                created_at: row.get(9)?,
                started_at: row.get(10)?,
                finished_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        },
    )
    .optional()
    .map_err(db_error)
}

fn select_event(conn: &Connection, id: &str) -> Result<Option<JobEventDto>> {
    conn.query_row(
        "SELECT id, job_id, seq, level, message, payload_json, created_at
         FROM job_events
         WHERE id = ?1",
        [id],
        event_from_row,
    )
    .optional()
    .map_err(db_error)
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobEventDto> {
    let payload_text: Option<String> = row.get(5)?;
    let payload = payload_text
        .map(|text| serde_json::from_str(&text).map_err(|err| to_sql_error(err.to_string())))
        .transpose()?;
    Ok(JobEventDto {
        id: row.get(0)?,
        job_id: row.get(1)?,
        seq: row.get(2)?,
        level: row.get(3)?,
        message: row.get(4)?,
        payload,
        created_at: row.get(6)?,
    })
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn db_error(err: rusqlite::Error) -> UsagiError {
    UsagiError::internal(err.to_string())
}

fn to_sql_error(message: String) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message,
    )))
}
