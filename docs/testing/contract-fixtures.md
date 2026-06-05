# Contract Fixtures and Test Plan

Status: draft v0.1  
Scope: Rails-side tests for stable `usagi-api` integration

---

## 0. Goal

Rails development must not require a live `usagi-api` process for most tests.

Use contract fixtures to test:

```text
request construction
response parsing
candidate persistence
job polling behavior
error handling
security signing
```

This keeps Rails tests fast and prevents every test run from becoming a distributed systems seminar. Nobody asked for that before coffee.

---

## 1. Fixture layout

The API repository publishes a flat fixture bundle under `fixtures/contracts/`.
Rails may copy these files into nested Rails fixture folders if that is more
ergonomic, but the field shapes should stay identical.

```text
fixtures/contracts/
  catalog_status.ready.json
  search_concepts.hybrid_rrf.request.json
  search_concepts.hybrid_rrf.response.json
  search_batch.hybrid_rrf.request.json
  search_batch.hybrid_rrf.response.json
  mapper_drugs_query.request.json
  mapper_drugs_query.response.json
  mapper_drugs_batch_job.request.json
  mapper_drugs_batch_job.response.json
  jobs_status.running.json
  jobs_results.mapper_success.json
  error.index_not_ready.json
```

---

## 2. Shared expectations

All engine responses should include or allow Rails to preserve:

```text
request_id
status/state
error.code when failed
error.message when failed
provenance when ranking/model/index dependent
```

Rails should parse unknown extra fields without failing.

Rails should fail clearly on missing required fields.

---

## 3. Catalog fixtures

### 3.1 `catalog/status.ready.json`

```json
{
  "status": "ready",
  "api_version": "0.1.0",
  "catalog": {
    "artifact_id": "athena-20250827-standard-v1",
    "vocabulary_version": "20250827",
    "concept_count": 1234567,
    "scope": {
      "standard_concept": "S",
      "invalid_reason": null
    },
    "manifest_path": "/data/catalog/manifest.json"
  }
}
```

### 3.2 `catalog/status.not_configured.json`

```json
{
  "status": "not_configured",
  "api_version": "0.1.0",
  "catalog": null,
  "message": "Catalog has not been built"
}
```

---

## 4. Search fixtures

### 4.1 `search/concepts.hybrid_rrf.request.json`

```json
{
  "q": "tramadol 50 mg capsule",
  "mode": "hybrid_rrf",
  "limit": 20,
  "filters": {
    "domain_id": ["Drug"]
  },
  "hybrid": {
    "rrf_k": 60,
    "lexical_top_k": 100,
    "sapbert_top_k": 100
  }
}
```

### 4.2 `search/concepts.hybrid_rrf.response.json`

```json
{
  "query": "tramadol 50 mg capsule",
  "mode": "hybrid_rrf",
  "results": [
    {
      "rank": 1,
      "concept": {
        "concept_id": 40162522,
        "concept_name": "Tramadol Hydrochloride 50 MG Oral Capsule",
        "domain_id": "Drug",
        "vocabulary_id": "RxNorm",
        "concept_class_id": "Clinical Drug",
        "standard_concept": "S",
        "concept_code": "859751"
      },
      "scores": {
        "tantivy": 12.83,
        "sapbert": 0.832,
        "rrf": 0.0318
      },
      "component_ranks": {
        "tantivy": 1,
        "sapbert": 4
      },
      "method": "hybrid_rrf"
    }
  ],
  "provenance": {
    "catalog_artifact_id": "athena-20250827-standard-v1",
    "model_artifact_id": "sapbert-xlmr-merged-v1",
    "index_artifact_id": "athena-20250827-hybrid-rrf-v1"
  }
}
```

### 4.3 `search/batch.hybrid_rrf.request.json`

```json
{
  "mode": "hybrid_rrf",
  "limit_per_item": 20,
  "filters": {
    "domain_id": ["Condition"]
  },
  "hybrid": {
    "rrf_k": 60,
    "lexical_top_k": 100,
    "sapbert_top_k": 100
  },
  "items": [
    {
      "id": "src_001",
      "q": "type 2 diabetes mellitus",
      "source_code": "DX001",
      "limit": 20
    }
  ]
}
```

### 4.4 `search/batch.hybrid_rrf.response.json`

```json
{
  "mode": "hybrid_rrf",
  "items": [
    {
      "id": "src_001",
      "results": [
        {
          "rank": 1,
          "concept": {
            "concept_id": 201826,
            "concept_name": "Type 2 diabetes mellitus",
            "domain_id": "Condition",
            "vocabulary_id": "SNOMED",
            "concept_class_id": "Clinical Finding",
            "standard_concept": "S",
            "concept_code": "44054006"
          },
          "scores": {
            "tantivy": 10.1,
            "sapbert": 0.891,
            "rrf": 0.0322
          },
          "component_ranks": {
            "tantivy": 1,
            "sapbert": 2
          },
          "method": "hybrid_rrf"
        }
      ]
    }
  ],
  "provenance": {
    "catalog_artifact_id": "athena-20250827-standard-v1",
    "model_artifact_id": "sapbert-xlmr-merged-v1",
    "index_artifact_id": "athena-20250827-hybrid-rrf-v1"
  }
}
```

---

## 5. Mapper fixtures

### 5.1 `mapper/drugs_query.request.json`

```json
{
  "source_name": "amoxicillin clavulanate 875 mg tablet",
  "source_code": "LOCAL123",
  "mode": "thirawat_tachiom",
  "candidate_top_k": 200,
  "rerank_top_n": 100,
  "limit": 20,
  "post_rank": {
    "mode": "tiebreak",
    "epsilon": 0.01,
    "top_n": 100
  }
}
```

### 5.2 `mapper/drugs_query.response.json`

```json
{
  "query": {
    "source_name": "amoxicillin clavulanate 875 mg tablet",
    "source_code": "LOCAL123",
    "query_text": "amoxicillin clavulanate 875 mg tablet (LOCAL123)"
  },
  "mode": "thirawat_tachiom",
  "candidates": [
    {
      "rank": 1,
      "concept": {
        "concept_id": 123456,
        "concept_name": "Amoxicillin / Clavulanate 875 MG Oral Tablet",
        "domain_id": "Drug",
        "vocabulary_id": "RxNorm",
        "concept_class_id": "Clinical Drug",
        "standard_concept": "S",
        "concept_code": "rx123"
      },
      "scores": {
        "tachiom_maxsim": 0.842,
        "bimaxsim": 0.913,
        "tie_breaker": 0.031,
        "final": 0.913
      },
      "features": {
        "ingredient_match": true,
        "strength_exact": true,
        "dose_form_match": true,
        "route_match": null,
        "release_match": null,
        "brand_match": null,
        "combination_count_match": true
      },
      "method": "thirawat_tachiom_bimaxsim_tiebreak"
    }
  ],
  "provenance": {
    "catalog_artifact_id": "athena-20250827-standard-v1",
    "model_artifact_id": "sidataplus/THIRAWAT-SapBERT",
    "index_artifact_id": "athena-20250827-thirawat-drug-tachiom-v1"
  }
}
```

### 5.3 `mapper/drugs_batch_job.response.json`

```json
{
  "job_id": "job_map_abc",
  "state": "queued",
  "status_url": "/jobs/job_map_abc",
  "result_url": "/jobs/job_map_abc/results"
}
```

---

## 6. Job fixtures

### 6.1 `jobs/status.running.json`

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
  "message": "Reranking candidates",
  "created_at": "2026-05-31T00:00:00Z",
  "started_at": "2026-05-31T00:00:02Z",
  "finished_at": null,
  "updated_at": "2026-05-31T00:10:00Z"
}
```

### 6.2 `jobs/status.succeeded_with_errors.json`

```json
{
  "id": "job_map_abc",
  "kind": "mapper_drugs_batch",
  "queue": "map",
  "state": "succeeded_with_errors",
  "stage": "done",
  "processed": 5000,
  "total": 5000,
  "failed": 3,
  "message": "Finished with item-level errors",
  "created_at": "2026-05-31T00:00:00Z",
  "started_at": "2026-05-31T00:00:02Z",
  "finished_at": "2026-05-31T00:20:00Z",
  "updated_at": "2026-05-31T00:20:00Z"
}
```

### 6.3 `jobs/results.mapper_success.json`

```json
{
  "job_id": "job_map_abc",
  "state": "succeeded",
  "artifact": {
    "artifact_id": "job_map_abc_results_v1",
    "path": "/data/jobs/results/job_map_abc/results.jsonl",
    "manifest_path": "/data/jobs/results/job_map_abc/manifest.json",
    "content_type": "application/jsonl",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "provenance": {
      "catalog_artifact_id": "athena-20250827-standard-v1",
      "model_artifact_id": "sidataplus/THIRAWAT-SapBERT",
      "index_artifact_id": "athena-20250827-thirawat-drug-tachiom-v1"
    }
  }
}
```

The referenced JSONL result rows contain one mapper batch item response per
source term.

### 6.4 `jobs/results.mapper_partial_failure.json`

```json
{
  "job_id": "job_map_abc",
  "state": "succeeded_with_errors",
  "items": [
    {
      "source_id": "src_001",
      "state": "succeeded",
      "candidates": []
    },
    {
      "id": "src_bad",
      "source_code": "SRC_BAD",
      "q": "query with no precomputed embedding",
      "error": {
        "code": "EMBEDDING_FAILED",
        "message": "Unable to encode source term",
        "details": {
          "source_code": "SRC_BAD"
        }
      }
    }
  ]
}
```

Live mapper JSONL may identify a failed item by `error` without an explicit
failed `state`. Rails should preserve `id`, `source_code`, `q`, and `error` in
`engine_jobs.error.items` while still persisting successful rows from the same
job.

---

## 7. Error fixtures

### 7.1 `errors/index_not_ready.json`

```json
{
  "error": {
    "code": "INDEX_NOT_READY",
    "message": "SapBERT index is not built",
    "details": {
      "required_artifact": "sapbert_cls.usearch"
    },
    "request_id": "req_abc"
  }
}
```

### 7.2 `errors/api_key_invalid.json`

```json
{
  "error": {
    "code": "UNAUTHORIZED",
    "message": "missing or invalid API key",
    "details": {},
    "request_id": "req_bad"
  }
}
```

---

## 8. Rails tests by feature

### 8.1 EngineClients::BaseClient

Test:

```text
adds Accept and Content-Type
adds request ID
adds signed headers
parses success JSON
parses error envelope
raises EngineClients::BaseClient::Error
preserves error code/message/details/request_id
handles invalid JSON
handles connection timeout
retries transient GET failures only
logs engine.request and engine.request.retry with request_id and attempt
```

### 8.2 EngineClients::SearchClient

Test:

```text
builds /search/concepts request
builds /search/batch request
passes hybrid defaults
supports domain/vocabulary filters
uses item id as the batch response join key
parses hybrid search results
uses source term domain hints for mixed project batch filters
```

### 8.3 EngineClients::MapperClient

Test:

```text
builds /mapper/drugs/query request
builds /mapper/drugs/batch-job request
uses idempotency key
uses THIRAWAT defaults
parses queued job response
records job creation failures as failed engine job mirrors
retries failed and partial mapper mirrors with the same idempotency key
does not treat succeeded_with_errors as clean reusable success
propagates artifact JSONL stream failures instead of returning empty items
copies item-level JSONL failures into engine_jobs.error.items
```

### 8.4 CandidatePersister

Test:

```text
persists hybrid candidates
persists mapper candidates
stores scores in correct columns
stores features/provenance JSON
upserts duplicates
links candidates to mapping and source term
handles failed items without aborting all successes
marks the engine job mirror failed when result streaming fails
```

### 8.5 ManualSearchesController

Test:

```text
creates a short-lived engine job mirror
marks the mirror succeeded with processed count and candidate count
marks the mirror failed with code/message/details/request_id on engine errors
does not crash the review page when the engine is unavailable
```

### 8.6 PollEngineJobJob

Test:

```text
updates running progress
re-enqueues when running
calls result persistence when succeeded
calls result persistence when succeeded_with_errors
stores failed error state
stops on cancelled
```

### 8.7 LiveEngineSmokeTest

Run only with `USAGI_LIVE_ENGINE=1` and live API URLs.

```text
status clients reach catalog/search/mapper
search concept query returns DTOs
search batch preserves returned item id and provenance
mapper query returns DTOs
mapper batch job can be created, polled, and streamed as JSONL
```

### 8.8 LiveWorkflowE2ETest

Run only with `USAGI_LIVE_ENGINE=1` and live API URLs.

```text
creates Drug and Mixed projects through Rails
imports source terms through Rails controllers
runs manual search and persists candidates
starts mapper batch auto-map, polls jobs API, and persists JSONL results
renders mapper partial-failure job details with retry
runs mixed-project hybrid auto-suggest through search batch
renders local hybrid job details without a jobs API mirror
shows persisted candidates on the review page
```

---

## 9. System test scenarios

Required MVP system tests:

```text
user logs in
user creates Drug project
user imports CSV
user reviews mapping table
user manually searches candidate
user applies candidate
user approves mapping
user exports STCM CSV
```

Drug mapper scenario with mocked API:

```text
user opens Drug project
clicks Suggest drug mappings
job card appears
mocked polling reaches succeeded
candidates appear in drawer
mapping remains UNCHECKED
```

Non-drug hybrid scenario with mocked API:

```text
user creates Condition project
imports condition terms
clicks Suggest candidates
hybrid job runs
candidates persist
```

Failure scenario:

```text
engine offline
review page still loads
manual search shows inline error
admin status page shows engine unavailable
```

Security scenario:

```text
valid API key request works
bad API key fixture maps to friendly error
```

---

## 10. Drift detection

Contract fixture tests should fail when:

```text
required fields disappear
status/state values change unexpectedly
candidate score fields change shape without mapper
error envelope shape changes
job state values change
```

Contract fixture tests should allow:

```text
extra response fields
extra provenance keys
extra score keys
new warning fields
```

This lets the API evolve without breaking Rails over harmless additions.

---

## 11. Fixture generation policy

When `usagi-api` changes an endpoint:

```text
1. update API docs
2. update contract fixtures
3. update Rails parser tests
4. update candidate persistence tests if needed
5. update system test mocks if needed
```

Do not silently update fixtures to make tests pass. That is not testing. That is laundering mistakes.
