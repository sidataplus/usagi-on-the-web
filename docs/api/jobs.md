# usagi-api jobs

Status: draft v0.1  
Audience: API implementers and Rails integrators  
Scope: SQLite-backed API job system for build, index, embedding, and bulk mapping work

## 1. Purpose

`usagi-api` uses asynchronous jobs for expensive or long-running work.

Async-only:

```text
catalog build
Tantivy index build
SapBERT CLS index build
THIRAWAT Drug document embedding build
Tachiom index build
Drug batch mapping
```

Synchronous endpoints exist only for small queries and smoke tests.

The API job system is not the Rails product workflow queue. Rails later uses Active Job and Solid Queue for user-facing workflow, then delegates heavy engine work to `usagi-api` jobs.

## 2. Job service model

```text
API endpoint
  -> creates job row in jobs.sqlite
  -> returns job_id immediately

api-worker
  -> claims queued jobs
  -> executes stages
  -> writes job_events
  -> writes job_items
  -> writes artifacts/results
  -> marks job succeeded/failed/cancelled
```

One worker binary is used first:

```text
usagi-worker --queues catalog,index,embed,map
```

Later, the same binary can be deployed with narrower queue selections.

## 3. Storage

```text
data/jobs/jobs.sqlite
data/jobs/results/
```

## 4. SQLite schema

```sql
CREATE TABLE jobs (
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

CREATE INDEX idx_jobs_state_queue
  ON jobs(state, queue, created_at);

CREATE INDEX idx_jobs_idempotency
  ON jobs(idempotency_key);

CREATE TABLE job_events (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL,
  seq INTEGER NOT NULL,
  level TEXT NOT NULL,
  message TEXT NOT NULL,
  payload_json TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX idx_job_events_seq
  ON job_events(job_id, seq);

CREATE TABLE job_items (
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

CREATE UNIQUE INDEX idx_job_items_key
  ON job_items(job_id, item_key);

CREATE INDEX idx_job_items_state
  ON job_items(job_id, state);
```

## 5. Job states

```text
queued
running
succeeded
succeeded_with_errors
failed
cancelled
```

## 6. Job item states

```text
queued
running
succeeded
failed
skipped
cancelled
```

## 7. State transitions

```text
queued
  -> running
  -> succeeded
  -> succeeded_with_errors
  -> failed
  -> cancelled

running
  -> succeeded
  -> succeeded_with_errors
  -> failed
  -> cancelled

failed
  -> queued     via retry endpoint

succeeded
  terminal

succeeded_with_errors
  terminal unless retry failed items is requested

cancelled
  terminal unless retry is requested
```

## 8. Queues

| Queue | Jobs | Default concurrency |
|---|---|---:|
| `catalog` | `catalog_build` | 1 |
| `index` | `tantivy_build`, `sapbert_build`, `tachiom_build` | 1 |
| `embed` | `sapbert_embed`, `thirawat_doc_embed` | 1 per model/device |
| `map` | `mapper_drugs_batch` | 1-2 |

The first implementation may use one process and one worker thread per queue. Correctness beats clever parallelism. Shocking that this still needs saying.

## 9. Job kinds

| Kind | Queue | Output |
|---|---|---|
| `catalog_build` | `catalog` | `catalog.sqlite`, catalog manifest |
| `tantivy_build` | `index` | Tantivy index, manifest |
| `sapbert_build` | `embed` then `index` | USearch SapBERT CLS index, manifest |
| `thirawat_doc_embed` | `embed` | THIRAWAT Drug token artifacts |
| `tachiom_build` | `index` | Tachiom index |
| `mapper_drugs_batch` | `map` | mapping result artifact |
| `translate_batch` | later | translation result artifact |

## 10. Job creation

All build and bulk endpoints create jobs.

Example:

```http
POST /mapper/drugs/batch-job
```

Request:

```json
{
  "idempotency_key": "project_123_auto_map_drug_v1",
  "mode": "thirawat_tachiom",
  "candidate_top_k": 200,
  "rerank_top_n": 100,
  "limit": 20,
  "items": [
    {
      "id": "src_001",
      "source_code": "SRC001",
      "source_name": "tramadol hydrochloride 50 mg capsule",
      "source_frequency": 376904
    }
  ]
}
```

Response:

```json
{
  "job_id": "job_map_abc",
  "state": "queued",
  "status_url": "/jobs/job_map_abc",
  "result_url": "/jobs/job_map_abc/results"
}
```

## 11. Idempotency

All build and bulk jobs require an idempotency key.

Recommended formula:

```text
sha256(
  job_kind
  + input_hash
  + catalog_artifact_id
  + model_artifact_id
  + index_artifact_id
  + mode
  + params
)
```

If an idempotency key already exists:

| Existing state | Behavior |
|---|---|
| `queued` | Return existing job |
| `running` | Return existing job |
| `succeeded` | Return existing job and result URL |
| `succeeded_with_errors` | Return existing job unless retry requested |
| `failed` | Return existing failed job unless retry requested |
| `cancelled` | Return existing cancelled job unless retry requested |

## 12. Worker claiming

Worker claims jobs by queue.

Pseudo-SQL:

```sql
BEGIN IMMEDIATE;

SELECT id
FROM jobs
WHERE state = 'queued'
  AND queue IN (...)
ORDER BY created_at
LIMIT 1;

UPDATE jobs
SET state = 'running',
    started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
    updated_at = CURRENT_TIMESTAMP
WHERE id = ?;

COMMIT;
```

SQLite job claiming should use `BEGIN IMMEDIATE` to prevent two workers claiming the same job. Yes, this is humble. Humble works.

## 13. Job events

Every meaningful stage writes a job event.

```json
{
  "job_id": "job_map_abc",
  "seq": 12,
  "level": "info",
  "message": "Reranking candidates",
  "payload": {
    "stage": "bimaxsim_reranking",
    "processed": 2400,
    "total": 5000
  },
  "created_at": "2026-05-31T00:10:00Z"
}
```

Event levels:

```text
debug
info
warn
error
```

## 14. Progress fields

Jobs maintain aggregate progress:

```text
processed
total
failed
stage
```

Rules:

- `processed` counts completed items or units of work.
- `failed` counts item-level failures.
- `total` should be set as early as possible.
- Stage names must be stable and documented per job kind.

## 15. Cancellation

`POST /jobs/:id/cancel` sets `cancel_requested = 1`.

Workers check cancellation between stages and between item batches.

If cancellation is observed:

```text
running -> cancelled
```

Partial artifacts from cancelled jobs must not be promoted to final artifact paths.

Temporary paths may be cleaned up unless debug retention is enabled.

## 16. Retry

`POST /jobs/:id/retry`

Request:

```json
{
  "failed_items_only": true
}
```

Behavior:

| Original job | Retry behavior |
|---|---|
| `failed` build job | create new job with same input |
| `succeeded_with_errors` itemized job | retry failed items only if requested |
| `cancelled` job | create new job with same input |
| `succeeded` job | return conflict unless force is explicitly supported later |

Retries create a new job ID. Do not mutate completed job history, because history is not a whiteboard no matter how much developers wish otherwise.

## 17. Result storage

Small job results may live in `jobs.result_json`.

Large job results must be artifact-backed.

```text
data/jobs/results/job_map_abc/
  results.jsonl
  manifest.json
```

`GET /jobs/:id/results` returns either inline result JSON or artifact reference.

Artifact response:

```json
{
  "job_id": "job_map_abc",
  "state": "succeeded",
  "artifact": {
    "artifact_id": "job_map_abc_results_v1",
    "path": "/data/jobs/results/job_map_abc/results.jsonl",
    "content_type": "application/jsonl",
    "sha256": "..."
  }
}
```

## 18. Partial failure

Bulk jobs must tolerate item-level failures.

Final state:

```text
succeeded_with_errors
```

when:

```text
processed = total
failed > 0
```

Each failed item gets `job_items.error_json`.

Example item error:

```json
{
  "code": "EMBEDDING_FAILED",
  "message": "Unable to encode source term",
  "details": {
    "source_code": "SRC_BAD"
  }
}
```

One malformed term should not destroy a 5,000-row mapping job. That is not robustness. That is a tantrum.

## 19. Job endpoint contracts

### 19.1 `GET /jobs/:id`

Response:

```json
{
  "id": "job_map_abc",
  "kind": "mapper_drugs_batch",
  "queue": "map",
  "state": "running",
  "stage": "bimaxsim_reranking",
  "processed": 2400,
  "total": 5000,
  "failed": 3,
  "created_at": "2026-05-31T00:00:00Z",
  "started_at": "2026-05-31T00:00:02Z",
  "finished_at": null,
  "updated_at": "2026-05-31T00:10:00Z"
}
```

### 19.2 `GET /jobs/:id/events`

Query parameters:

| Parameter | Type | Default |
|---|---|---:|
| `after_seq` | integer | `0` |
| `limit` | integer | `100` |

Response:

```json
{
  "job_id": "job_map_abc",
  "events": [
    {
      "seq": 12,
      "level": "info",
      "message": "Reranking candidates",
      "payload": {
        "stage": "bimaxsim_reranking",
        "processed": 2400,
        "total": 5000
      },
      "created_at": "2026-05-31T00:10:00Z"
    }
  ]
}
```

### 19.3 `GET /jobs/:id/results`

Response with artifact:

```json
{
  "job_id": "job_map_abc",
  "state": "succeeded",
  "artifact": {
    "artifact_id": "job_map_abc_results_v1",
    "path": "/data/jobs/results/job_map_abc/results.jsonl",
    "content_type": "application/jsonl",
    "sha256": "..."
  }
}
```

### 19.4 `POST /jobs/:id/cancel`

Response:

```json
{
  "job_id": "job_map_abc",
  "state": "cancel_requested"
}
```

### 19.5 `POST /jobs/:id/retry`

Request:

```json
{
  "failed_items_only": true
}
```

Response:

```json
{
  "job_id": "job_map_retry_def",
  "state": "queued",
  "status_url": "/jobs/job_map_retry_def",
  "result_url": "/jobs/job_map_retry_def/results"
}
```

## 20. Job kind specifications

### 20.1 `catalog_build`

Input:

```json
{
  "athena_dir": "/fixtures/athena-mini",
  "overwrite": false
}
```

Stages:

```text
validating_input
reading_athena_files
filtering_standard_concepts
writing_sqlite
creating_indexes
writing_manifest
validating_artifact
```

Output:

```json
{
  "catalog_artifact_id": "athena-20250827-standard-v1",
  "concept_count": 1234567,
  "manifest_path": "/data/catalog/manifest.json"
}
```

Failure conditions:

- Missing `CONCEPT` file
- Invalid delimiter/header
- No standard concepts loaded
- SQLite write failure
- Manifest checksum failure

### 20.2 `tantivy_build`

Input:

```json
{
  "catalog_artifact_id": "athena-20250827-standard-v1",
  "schema_version": "usagi-tantivy-v1",
  "overwrite": false
}
```

Stages:

```text
validating_catalog
reading_concepts
building_index
committing_index
writing_manifest
validating_artifact
```

Output:

```json
{
  "tantivy_artifact_id": "athena-20250827-tantivy-v1",
  "document_count": 1234567
}
```

### 20.3 `sapbert_build`

Input:

```json
{
  "catalog_artifact_id": "athena-20250827-standard-v1",
  "model_artifact_id": "sapbert-xlmr-merged-v1",
  "batch_size": 32,
  "overwrite": false
}
```

Stages:

```text
validating_catalog
validating_model
loading_model
reading_concepts
embedding_cls
building_usearch_index
writing_concept_ids
writing_manifest
validating_artifact
```

Output:

```json
{
  "sapbert_artifact_id": "athena-20250827-sapbert-cls-v1",
  "document_count": 1234567
}
```

### 20.4 `thirawat_doc_embed`

Input:

```json
{
  "catalog_artifact_id": "athena-20250827-standard-v1",
  "model_artifact_id": "sidataplus-thirawat-sapbert-merged-v1",
  "domain_id": "Drug",
  "batch_size": 16,
  "target_scope": {
    "domain_id": ["Drug"],
    "vocabulary_id": ["RxNorm", "RxNorm Extension"]
  },
  "overwrite": false
}
```

Stages:

```text
validating_catalog
validating_model
loading_model
selecting_drug_targets
building_document_text
embedding_documents
writing_token_vectors
writing_doc_ids
writing_manifest
validating_artifact
```

Output:

```json
{
  "thirawat_doc_embedding_artifact_id": "athena-20250827-thirawat-drug-docemb-v1",
  "documents": 123456,
  "tokens": 9876543
}
```

### 20.5 `tachiom_build`

Input:

```json
{
  "doc_embedding_artifact_id": "athena-20250827-thirawat-drug-docemb-v1",
  "build_params": {
    "metric": "maxsim",
    "token_aware_clustering": true
  },
  "overwrite": false
}
```

Stages:

```text
validating_doc_embeddings
loading_token_vectors
building_tachiom_index
writing_index
writing_manifest
validating_artifact
```

Output:

```json
{
  "tachiom_artifact_id": "athena-20250827-thirawat-drug-tachiom-v1"
}
```

### 20.6 `mapper_drugs_batch`

Input:

```json
{
  "mode": "thirawat_tachiom",
  "candidate_top_k": 200,
  "rerank_top_n": 100,
  "limit": 20,
  "items": [
    {
      "id": "src_001",
      "source_code": "SRC001",
      "source_name": "tramadol hydrochloride 50 mg capsule",
      "source_frequency": 376904
    }
  ]
}
```

Stages:

```text
validating_indexes
normalizing
embedding_queries
tachiom_retrieval
bimaxsim_reranking
deterministic_tiebreak
writing_results
validating_results
```

Output:

```json
{
  "result_artifact_id": "job_map_abc_results_v1",
  "processed": 5000,
  "failed": 3
}
```

## 21. Worker configuration

Environment variables:

```text
JOBS_DB_PATH=/data/jobs/jobs.sqlite
WORKER_QUEUES=catalog,index,embed,map
WORKER_CONCURRENCY=1
EMBED_DEVICE=cpu
CATALOG_DB_PATH=/data/catalog/catalog.sqlite
TANTIVY_INDEX_DIR=/data/search/tantivy/index
SAPBERT_INDEX_DIR=/data/search/sapbert
THIRAWAT_ARTIFACT_DIR=/data/mapper/thirawat-drug
```

## 22. Observability

Each worker log line includes:

```text
request_id
job_id
kind
queue
state
stage
duration_ms
processed
total
failed
artifact_id
error_code
```

Required metrics:

```text
jobs_created_total
jobs_succeeded_total
jobs_failed_total
jobs_cancelled_total
job_duration_seconds
job_stage_duration_seconds
job_items_failed_total
embedding_items_per_second
mapper_items_per_second
```

## 23. Testing gates

| Test | Required behavior |
|---|---|
| Job claim race | Two workers do not claim same job |
| Cancel | Running job observes cancel request |
| Retry | Failed job can create retry job |
| Idempotency | Duplicate request returns same job |
| Partial failure | Failed item does not fail entire bulk job |
| Result artifact | Large result written and checksummed |
| Manifest validation | Invalid artifact fails job |
| Restart | Worker restart resumes queued/running-safe jobs |

## 24. Rails integration expectations

Rails later will mirror API jobs in an `engine_jobs` table.

Rails will poll:

```text
GET /jobs/:id
GET /jobs/:id/events
GET /jobs/:id/results
```

Rails will not directly read `jobs.sqlite`.

Rails workflow jobs are separate and will use Active Job plus Solid Queue. API jobs remain engine-level jobs. Two queues, two responsibilities. Madness postponed.
