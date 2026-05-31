# mapper-api Manual

Status: draft v0.1  
Audience: API operators, Rails integrators, and developers

## Purpose

`mapper-api` owns Drug-domain mapping:

```text
THIRAWAT-SapBERT query embedding
Tachiom MaxSim retrieval
external exact BiMaxSim rerank
deterministic drug tie-breaker
synchronous drug query and small batch mapping
asynchronous project-scale drug batch mapping
```

API v0.1 supports Drug mapping only.

## Ownership

`mapper-api` owns:

```text
data/mapper/thirawat-drug/doc_embeddings/
data/mapper/thirawat-drug/tachiom/
mapper query and batch responses
mapper batch job creation
mapper job polling endpoints
```

It depends on THIRAWAT model artifacts and Tachiom indexes. Rails owns projects,
source terms, review state, and final mapping decisions.

## Runtime Configuration

| Variable | Default | Meaning |
|---|---|---|
| `MAPPER_API_ADDR` | `0.0.0.0:8790` | Bind address |
| `JOBS_DB_PATH` | `data/jobs/jobs.sqlite` | Shared API job store |
| `JOB_RESULTS_DIR` | `data/jobs/results` | Result artifact root |
| `THIRAWAT_MODEL_DIR` | `data/models/thirawat-sapbert` | Merged THIRAWAT model artifact |
| `THIRAWAT_ARTIFACT_DIR` | `data/mapper/thirawat-drug` | Mapper artifact root |
| `TACHIOM_INDEX_DIR` | `data/mapper/thirawat-drug/tachiom` | Tachiom artifact |
| `THIRAWAT_QUERY_EMBEDDINGS_PATH` | unset | Precomputed query fixture path |
| `TACHIOM_SEARCH_BIN` | unset | Native Tachiom search binary for CLI backend |
| `USAGI_API_KEYS` | none, fail closed | Comma-separated API keys |
| `USAGI_API_BODY_LIMIT_BYTES` | `262144` | JSON request body limit |

## Endpoints

Public probes:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/mapper/health` | Liveness |
| `GET` | `/mapper/status` | Mapper readiness |

Protected endpoints:

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/mapper/thirawat/build-embeddings-job` | Build Drug document token artifacts |
| `POST` | `/mapper/tachiom/build-index-job` | Build Tachiom index |
| `POST` | `/mapper/drugs/query` | Map one Drug source string |
| `POST` | `/mapper/drugs/batch` | Map a small batch synchronously |
| `POST` | `/mapper/drugs/batch-job` | Create async mapper batch job |
| `POST` | `/mapper/drugs/explain` | Explain one source/concept pair |
| `GET` | `/jobs/:id` | Job status |
| `GET` | `/jobs/:id/events` | Job event log |
| `GET` | `/jobs/:id/results` | Job result metadata or JSONL stream |
| `POST` | `/jobs/:id/cancel` | Cancel queued/running job |
| `POST` | `/jobs/:id/retry` | Retry eligible job |

## Build Mapper Artifacts

Create document embedding build job:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{"idempotency_key":"thirawat-docemb-local-v1"}' \
  http://127.0.0.1:8790/mapper/thirawat/build-embeddings-job
```

Create Tachiom build job:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{"idempotency_key":"tachiom-local-v1"}' \
  http://127.0.0.1:8790/mapper/tachiom/build-index-job
```

Run worker queues:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues embed,index --once
```

## Query Examples

Single Drug query:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "source_name": "tramadol hydrochloride 50 mg capsule",
    "source_code": "SRC_TRAMADOL_50_CAP",
    "mode": "thirawat_tachiom",
    "candidate_top_k": 200,
    "rerank_top_n": 100,
    "limit": 20,
    "post_rank": {"mode": "tiebreak", "epsilon": 0.01, "top_n": 100}
  }' \
  http://127.0.0.1:8790/mapper/drugs/query
```

Async batch job:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "idempotency_key": "project_123_mapper_drugs_v1",
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
  }' \
  http://127.0.0.1:8790/mapper/drugs/batch-job
```

## Result Streaming

`GET /jobs/:id/results` returns metadata by default:

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

Rails should request the actual JSONL body over HTTP:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Accept: application/jsonl' \
  http://127.0.0.1:8790/jobs/job_map_abc/results
```

The service validates the artifact manifest checksum before streaming the body.

## Rails Integration Notes

Rails should call `mapper-api` only for Drug-domain rows in API v0.1. Rails
stores mapper candidates and provenance, but Rails owns final approval and export
state.

Candidate provenance uses:

```json
{
  "catalog_artifact_id": "local-catalog-standard-v1",
  "model_artifact_id": "sidataplus/THIRAWAT-SapBERT",
  "index_artifact_id": "local-thirawat-drug-tachiom-v1"
}
```

## Verification

Run:

```bash
cargo test -p usagi-thirawat -p usagi-tachiom
scripts/smoke-mapper-fixture.sh
scripts/smoke-mapper-batch-fixture.sh
scripts/smoke-dev-drugs-50.sh
scripts/smoke-dev-drugs-50-batch-job.sh
```

Use `scripts/smoke-native-tachiom-dev-drugs-50.sh` when native upstream Tachiom
CLI binaries are available.
