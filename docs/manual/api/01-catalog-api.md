# catalog-api Manual

Status: draft v0.1  
Audience: API operators, Rails integrators, and developers

## Purpose

`catalog-api` owns the runtime OMOP catalog. Runtime catalog truth is SQLite
only and contains valid standard concepts only:

```sql
standard_concept = 'S'
AND invalid_reason IS NULL
```

DuckDB may be used for fixture extraction and development bootstrap work, but it
is not a runtime dependency of this service.

## Ownership

`catalog-api` owns:

```text
data/catalog/catalog.sqlite
data/catalog/manifest.json
standard concept lookup
batch concept lookup
hierarchy and relationship queries
reference-table endpoints
catalog build job creation
catalog job polling endpoints for jobs it creates
```

It does not own search indexes, mapper indexes, Rails projects, user mappings,
or review state.

## Runtime Configuration

| Variable | Default | Meaning |
|---|---|---|
| `CATALOG_API_ADDR` | `0.0.0.0:8788` | Bind address |
| `CATALOG_DB_PATH` | `data/catalog/catalog.sqlite` | Runtime SQLite catalog |
| `JOBS_DB_PATH` | `data/jobs/jobs.sqlite` | Shared API job store |
| `JOB_RESULTS_DIR` | `data/jobs/results` in Compose | Shared result artifact root |
| `USAGI_API_KEYS` | none, fail closed | Comma-separated API keys |
| `USAGI_API_BODY_LIMIT_BYTES` | `262144` | JSON request body limit |

Docker Compose maps the container paths to `/data/...`.

## Endpoints

Public probes:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/catalog/health` | Liveness |
| `GET` | `/catalog/status` | Catalog readiness and manifest summary |

Protected endpoints:

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/catalog/build-job` | Create async catalog build job |
| `GET` | `/catalog/concepts/:concept_id` | Concept detail |
| `POST` | `/catalog/concepts/batch` | Batch concept lookup |
| `GET` | `/catalog/concepts/:concept_id/ancestors` | Ancestor concepts |
| `GET` | `/catalog/concepts/:concept_id/descendants` | Descendant concepts |
| `GET` | `/catalog/concepts/:concept_id/relationships` | Direct relationships |
| `GET` | `/catalog/domains` | Domains |
| `GET` | `/catalog/vocabularies` | Vocabularies |
| `GET` | `/catalog/concept-classes` | Concept classes |
| `GET` | `/jobs/:id` | Job status |
| `GET` | `/jobs/:id/events` | Job event log |
| `GET` | `/jobs/:id/results` | Job result or artifact metadata |
| `POST` | `/jobs/:id/cancel` | Cancel queued/running job |
| `POST` | `/jobs/:id/retry` | Retry eligible job |

## Build Catalog

Create a build job:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{"athena_dir":"/fixtures/search-smoke/athena-mini","idempotency_key":"catalog-smoke-v1"}' \
  http://127.0.0.1:8788/catalog/build-job
```

Response:

```json
{
  "job_id": "job_catalog_abc",
  "state": "queued",
  "status_url": "/jobs/job_catalog_abc",
  "result_url": "/jobs/job_catalog_abc/results"
}
```

Run a worker with the `catalog` queue:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues catalog --once
```

Poll:

```bash
curl -fsS -H 'X-API-Key: local-secret' \
  http://127.0.0.1:8788/jobs/job_catalog_abc
```

## Lookup Examples

Concept detail:

```bash
curl -fsS -H 'X-API-Key: local-secret' \
  http://127.0.0.1:8788/catalog/concepts/40162522
```

Batch concept lookup:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{"concept_ids":[40162522,100]}' \
  http://127.0.0.1:8788/catalog/concepts/batch
```

Relationships:

```bash
curl -fsS -H 'X-API-Key: local-secret' \
  'http://127.0.0.1:8788/catalog/concepts/40162522/relationships?direction=both'
```

## Rails Integration Notes

Rails should use `catalog-api` for read-only vocabulary metadata and admin
catalog build orchestration. Rails must not read `catalog.sqlite` directly.

Good Rails cache candidates:

```text
/catalog/status
/catalog/domains
/catalog/vocabularies
/catalog/concept-classes
```

Do not cache mapping decisions as catalog truth.

## Verification

Catalog behavior is covered by:

```bash
cargo test -p usagi-catalog
scripts/smoke-search-fixture.sh
```

The search smoke builds the tiny Athena fixture through `catalog-api` and asserts
that downstream search only sees standard concepts.
