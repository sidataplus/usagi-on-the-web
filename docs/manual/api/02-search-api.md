# search-api Manual

Status: draft v0.1  
Audience: API operators, Rails integrators, and developers

## Purpose

`search-api` owns OMOP concept search:

```text
Tantivy lexical search
SapBERT CLS dense search
hybrid RRF fusion
search explanation
search index build job creation
```

Search results must be valid standard concepts from the runtime catalog.

## Ownership

`search-api` owns:

```text
data/search/tantivy/index/
data/search/sapbert/
search result provenance
Tantivy build job creation
SapBERT build job creation
search job polling endpoints for jobs it creates
```

It depends on `catalog.sqlite` for concept hydration and filtering. It does not
own catalog builds, mapper artifacts, Rails projects, or mapping decisions.

## Runtime Configuration

| Variable | Default | Meaning |
|---|---|---|
| `SEARCH_API_ADDR` | `0.0.0.0:8789` | Bind address |
| `CATALOG_DB_PATH` | `data/catalog/catalog.sqlite` | Runtime catalog |
| `JOBS_DB_PATH` | `data/jobs/jobs.sqlite` | Shared API job store |
| `TANTIVY_INDEX_DIR` | `data/search/tantivy/index` | Tantivy artifact |
| `SAPBERT_INDEX_DIR` | `data/search/sapbert` | SapBERT dense artifact |
| `SAPBERT_MODEL_DIR` | `data/models/sapbert` | SapBERT model artifact |
| `SAPBERT_MAX_LENGTH` | `96` | SapBERT sequence length |
| `SAPBERT_QUERY_EMBEDDINGS_PATH` | unset | Precomputed query fixture path |
| `USAGI_API_KEYS` | none, fail closed | Comma-separated API keys |
| `USAGI_API_BODY_LIMIT_BYTES` | `262144` | JSON request body limit |

## Endpoints

Public probes:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/search/health` | Liveness |
| `GET` | `/search/status` | Index readiness |

Protected endpoints:

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/search/tantivy/build-job` | Create Tantivy build job |
| `POST` | `/search/sapbert/build-job` | Create SapBERT build job |
| `POST` | `/search/concepts` | Search one query |
| `POST` | `/search/batch` | Search multiple queries |
| `POST` | `/search/explain` | Explain a query/concept score |
| `GET` | `/jobs/:id` | Job status |
| `GET` | `/jobs/:id/events` | Job event log |
| `GET` | `/jobs/:id/results` | Job result or artifact metadata |
| `POST` | `/jobs/:id/cancel` | Cancel queued/running job |
| `POST` | `/jobs/:id/retry` | Retry eligible job |

## Search Modes

| Mode | Requires | Notes |
|---|---|---|
| `lexical_tantivy` | Tantivy index | Fast text search |
| `sapbert_cls` | SapBERT dense artifact | Dense semantic search |
| `hybrid_rrf` | Tantivy and SapBERT | Deterministic reciprocal-rank fusion |

## Build Indexes

Create Tantivy build job:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{"idempotency_key":"tantivy-local-v1"}' \
  http://127.0.0.1:8789/search/tantivy/build-job
```

Create SapBERT build job:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{"idempotency_key":"sapbert-local-v1"}' \
  http://127.0.0.1:8789/search/sapbert/build-job
```

Run the worker:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues index,embed --once
```

For fixture builds, set `SAPBERT_PRECOMPUTED_EMBEDDINGS_PATH` for the worker and
`SAPBERT_QUERY_EMBEDDINGS_PATH` for `search-api`.

## Query Examples

Hybrid search:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "q": "tramadol 50 mg capsule",
    "mode": "hybrid_rrf",
    "limit": 20,
    "filters": {"domain_id": ["Drug"]},
    "hybrid": {"rrf_k": 60, "lexical_top_k": 100, "sapbert_top_k": 100}
  }' \
  http://127.0.0.1:8789/search/concepts
```

Batch search:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "mode": "lexical_tantivy",
    "limit_per_item": 5,
    "items": [{"id": "src_001", "q": "tramadol 50 mg capsule"}]
  }' \
  http://127.0.0.1:8789/search/batch
```

Explain:

```bash
curl -fsS \
  -H 'X-API-Key: local-secret' \
  -H 'Content-Type: application/json' \
  -d '{"q":"tramadol 50 mg capsule","mode":"hybrid_rrf","concept_id":100}' \
  http://127.0.0.1:8789/search/explain
```

## Rails Integration Notes

Rails should use `search-api` for manual concept search and non-drug candidate
lookup workflows. Search responses include provenance:

```json
{
  "catalog_artifact_id": "local-catalog-standard-v1",
  "model_artifact_id": "sapbert-precomputed-fixture-v1",
  "index_artifact_id": "local-hybrid-rrf-v1"
}
```

Rails may persist search candidates, but Rails owns mapping approval state.

## Verification

Run:

```bash
cargo test -p usagi-search
scripts/smoke-search-fixture.sh
```

The fixture smoke builds catalog, Tantivy, and SapBERT fixture artifacts, then
checks SapBERT, lexical, hybrid, batch provenance, and explain behavior.
