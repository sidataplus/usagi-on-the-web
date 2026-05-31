# usagi-api endpoints

Status: draft v0.1  
Audience: API implementers and Rails integrators  
Scope: API-first Rust services before Rails front-end development

## 1. Purpose

`usagi-api` provides the stable engine layer for Usagi v3.

```text
catalog-api
  SQLite OMOP vocabulary truth, standard concepts only

search-api
  Tantivy lexical search
  SapBERT CLS dense search with USearch
  Hybrid RRF search

mapper-api
  THIRAWAT-SapBERT Drug mapper
  Tachiom MaxSim-style retrieval
  External exact BiMaxSim rerank
  Deterministic near-tie resolver

api-worker
  Long-running build, index, embedding, and bulk mapping jobs

translate-api
  Optional later module after Rails front-end completion
```

Rails will later consume these endpoints and own projects, imports, mapping review, exports, users, permissions, and UI. The API layer owns vocabulary/search/model/retrieval work.

## 2. Global conventions

### 2.1 Transport

All endpoints use JSON over HTTP.

Default content type:

```http
Content-Type: application/json
Accept: application/json
```

Large job results may be returned as JSONL artifacts referenced by path or artifact ID.

### 2.2 Request IDs

All services must attach or propagate a request ID.

Accepted request header:

```http
X-Request-Id: req_...
```

If omitted, the service generates one.

All responses include:

```http
X-Request-Id: req_...
```

### 2.2.1 API Keys

Deployments require API keys by setting `USAGI_API_KEYS` to a comma-separated list of accepted keys. The API-only Docker Compose file fails closed if `USAGI_API_KEYS` is not set and binds published ports to `127.0.0.1` unless `USAGI_PUBLISH_HOST` is explicitly overridden. All non-probe endpoints require one of:

```http
X-API-Key: ...
Authorization: Bearer ...
```

Health and status endpoints remain unauthenticated for local and orchestration probes. All other endpoints fail closed when `USAGI_API_KEYS` is unset or empty, including when running a service binary directly.

### 2.3 Error envelope

All services use the same error shape.

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

Standard error codes:

| Code | Meaning |
|---|---|
| `BAD_REQUEST` | Request validation failed |
| `UNAUTHORIZED` | Missing or invalid API key |
| `NOT_FOUND` | Requested resource does not exist |
| `CATALOG_NOT_READY` | Catalog artifact is missing or invalid |
| `INDEX_NOT_READY` | Search or mapper index is missing or invalid |
| `MODEL_NOT_READY` | Required model artifact is missing or invalid |
| `INCOMPATIBLE_ARTIFACT` | Artifact manifest does not match catalog/model/API version |
| `JOB_NOT_FOUND` | Job ID not found |
| `JOB_CANCELLED` | Job was cancelled |
| `JOB_FAILED` | Job failed |
| `EMBEDDING_FAILED` | Candle embedding step failed |
| `TACHIOM_FAILED` | Tachiom retrieval or index build failed |
| `INTERNAL_ERROR` | Unexpected service failure |

### 2.4 Status values

Service status values:

```text
ready
not_configured
building
degraded
error
```

Job state values:

```text
queued
running
succeeded
succeeded_with_errors
failed
cancelled
```

### 2.5 Standard concept guarantee

The runtime catalog contains only valid standard Athena concepts.

```sql
standard_concept = 'S'
AND invalid_reason IS NULL
```

All search and mapper results must satisfy this standard concept guarantee unless an endpoint explicitly documents otherwise. API v0.1 does not expose non-standard concept lookup.

### 2.6 Response provenance

Search and mapper responses include artifact provenance whenever ranking depends on indexes or models.

```json
{
  "provenance": {
    "catalog_artifact_id": "athena-20250827-standard-v1",
    "model_artifact_id": "sidataplus-thirawat-sapbert-merged-v1",
    "index_artifact_id": "athena-20250827-thirawat-drug-tachiom-v1"
  }
}
```

## 3. Endpoint summary

### 3.1 Catalog API

| Method | Path | Purpose | Async |
|---|---|---:|---:|
| `GET` | `/catalog/health` | Liveness check | no |
| `GET` | `/catalog/status` | Catalog readiness and manifest | no |
| `POST` | `/catalog/build-job` | Build `catalog.sqlite` from Athena files | yes |
| `GET` | `/catalog/concepts/:concept_id` | Get concept detail | no |
| `POST` | `/catalog/concepts/batch` | Batch concept detail lookup | no |
| `GET` | `/catalog/concepts/:concept_id/ancestors` | Ancestor concepts | no |
| `GET` | `/catalog/concepts/:concept_id/descendants` | Descendant concepts | no |
| `GET` | `/catalog/concepts/:concept_id/relationships` | Direct relationships | no |
| `GET` | `/catalog/domains` | Domain list | no |
| `GET` | `/catalog/vocabularies` | Vocabulary list | no |
| `GET` | `/catalog/concept-classes` | Concept class list | no |

### 3.2 Search API

| Method | Path | Purpose | Async |
|---|---|---:|---:|
| `GET` | `/search/health` | Liveness check | no |
| `GET` | `/search/status` | Search index readiness | no |
| `POST` | `/search/tantivy/build-job` | Build Tantivy lexical index | yes |
| `POST` | `/search/sapbert/build-job` | Build SapBERT CLS + USearch index | yes |
| `POST` | `/search/concepts` | Search concepts | no |
| `POST` | `/search/batch` | Batch search concepts | no |
| `POST` | `/search/explain` | Explain scoring for one query | no |

### 3.3 Mapper API

| Method | Path | Purpose | Async |
|---|---|---:|---:|
| `GET` | `/mapper/health` | Liveness check | no |
| `GET` | `/mapper/status` | Mapper readiness | no |
| `POST` | `/mapper/thirawat/build-embeddings-job` | Build THIRAWAT Drug document token artifacts | yes |
| `POST` | `/mapper/tachiom/build-index-job` | Build Tachiom index from THIRAWAT artifacts | yes |
| `POST` | `/mapper/drugs/query` | Map one Drug source string | no |
| `POST` | `/mapper/drugs/batch` | Map a small batch synchronously | no |
| `POST` | `/mapper/drugs/batch-job` | Map a project-scale Drug batch | yes |
| `POST` | `/mapper/drugs/explain` | Detailed mapper explanation | no |

### 3.4 Jobs API

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/jobs/:id` | Get job status |
| `GET` | `/jobs/:id/events` | Get job event log |
| `GET` | `/jobs/:id/results` | Get final job result or artifact reference |
| `POST` | `/jobs/:id/cancel` | Request cancellation |
| `POST` | `/jobs/:id/retry` | Retry failed job or failed items |

### 3.5 Translation API, later optional module

The translation module is explicitly deferred until after the Rails front-end is complete.

| Method | Path | Purpose | Status |
|---|---|---:|---|
| `GET` | `/translate/status` | Translation readiness | deferred |
| `POST` | `/translate/text` | Translate one text | deferred |
| `POST` | `/translate/batch` | Translate small batch | deferred |
| `POST` | `/translate/batch-job` | Translate large batch | deferred |

## 4. Shared DTOs

### 4.1 Concept DTO

```json
{
  "concept_id": 40162522,
  "concept_name": "Tramadol Hydrochloride 50 MG Oral Capsule",
  "domain_id": "Drug",
  "vocabulary_id": "RxNorm",
  "concept_class_id": "Clinical Drug",
  "standard_concept": "S",
  "concept_code": "859751",
  "valid_start_date": "20000101",
  "valid_end_date": "20991231",
  "invalid_reason": null
}
```

### 4.2 Concept search result DTO

```json
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
```

### 4.3 Mapping candidate DTO

```json
{
  "rank": 1,
  "concept": {
    "concept_id": 123456,
    "concept_name": "Amoxicillin / Clavulanate 875 MG Oral Tablet",
    "domain_id": "Drug",
    "vocabulary_id": "RxNorm",
    "concept_class_id": "Clinical Drug",
    "standard_concept": "S",
    "concept_code": "..."
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
```

### 4.4 Job DTO

```json
{
  "id": "job_abc",
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

## 5. Catalog API details

### 5.1 `GET /catalog/health`

Response:

```json
{
  "status": "ok",
  "service": "catalog-api",
  "api_version": "0.1.0"
}
```

### 5.2 `GET /catalog/status`

Response when ready:

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

Response when not configured:

```json
{
  "status": "not_configured",
  "api_version": "0.1.0",
  "catalog": null,
  "message": "Catalog has not been built"
}
```

### 5.3 `POST /catalog/build-job`

Request:

```json
{
  "athena_dir": "/fixtures/athena-mini",
  "idempotency_key": "catalog-athena-mini-standard-v1",
  "overwrite": false
}
```

Response:

```json
{
  "job_id": "job_catalog_abc",
  "state": "queued",
  "status_url": "/jobs/job_catalog_abc"
}
```

Rules:

- Async only.
- Loads only standard valid concepts.
- Writes `catalog.sqlite` and `manifest.json`.
- If `overwrite = false` and catalog exists, return conflict unless idempotency key matches the existing job/artifact.

### 5.4 `GET /catalog/concepts/:concept_id`

Response:

```json
{
  "concept": {
    "concept_id": 40162522,
    "concept_name": "Tramadol Hydrochloride 50 MG Oral Capsule",
    "domain_id": "Drug",
    "vocabulary_id": "RxNorm",
    "concept_class_id": "Clinical Drug",
    "standard_concept": "S",
    "concept_code": "859751",
    "valid_start_date": "20000101",
    "valid_end_date": "20991231",
    "invalid_reason": null
  },
  "provenance": {
    "catalog_artifact_id": "athena-20250827-standard-v1"
  }
}
```

### 5.5 `POST /catalog/concepts/batch`

Request:

```json
{
  "concept_ids": [40162522, 1503297]
}
```

Response:

```json
{
  "concepts": [
    {
      "concept_id": 40162522,
      "concept_name": "Tramadol Hydrochloride 50 MG Oral Capsule",
      "domain_id": "Drug",
      "vocabulary_id": "RxNorm",
      "concept_class_id": "Clinical Drug",
      "standard_concept": "S",
      "concept_code": "859751"
    }
  ],
  "missing": []
}
```

### 5.6 Hierarchy endpoints

Paths:

```text
GET /catalog/concepts/:concept_id/ancestors
GET /catalog/concepts/:concept_id/descendants
```

Query parameters:

| Parameter | Type | Default |
|---|---|---:|
| `limit` | integer | `100` |
| `offset` | integer | `0` |
| `min_level` | integer | `1` |
| `max_level` | integer or null | null |

Response:

```json
{
  "concept_id": 40162522,
  "direction": "ancestors",
  "items": [
    {
      "concept": {
        "concept_id": 1125315,
        "concept_name": "Tramadol",
        "domain_id": "Drug",
        "vocabulary_id": "RxNorm",
        "concept_class_id": "Ingredient",
        "standard_concept": "S",
        "concept_code": "10689"
      },
      "min_levels_of_separation": 1,
      "max_levels_of_separation": 1
    }
  ]
}
```

### 5.7 Relationship endpoint

Path:

```text
GET /catalog/concepts/:concept_id/relationships
```

Query parameters:

| Parameter | Type |
|---|---|
| `relationship_id` | string, optional |
| `direction` | `outgoing`, `incoming`, `both` |

Response:

```json
{
  "concept_id": 40162522,
  "relationships": [
    {
      "relationship_id": "Has ingredient",
      "direction": "outgoing",
      "target_concept": {
        "concept_id": 1125315,
        "concept_name": "Tramadol",
        "domain_id": "Drug",
        "vocabulary_id": "RxNorm",
        "concept_class_id": "Ingredient",
        "standard_concept": "S",
        "concept_code": "10689"
      }
    }
  ]
}
```

## 6. Search API details

### 6.1 `GET /search/status`

Response:

```json
{
  "status": "ready",
  "api_version": "0.1.0",
  "catalog": {
    "artifact_id": "athena-20250827-standard-v1"
  },
  "indexes": {
    "tantivy": {
      "status": "ready",
      "artifact_id": "athena-20250827-tantivy-v1",
      "document_count": 1234567
    },
    "sapbert": {
      "status": "ready",
      "artifact_id": "athena-20250827-sapbert-cls-v1",
      "document_count": 1234567,
      "model_id": "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR"
    }
  }
}
```

### 6.2 `POST /search/tantivy/build-job`

Request:

```json
{
  "idempotency_key": "tantivy-athena-20250827-standard-v1",
  "overwrite": false,
  "schema_version": "usagi-tantivy-v1"
}
```

Response:

```json
{
  "job_id": "job_tantivy_abc",
  "state": "queued",
  "status_url": "/jobs/job_tantivy_abc"
}
```

### 6.3 `POST /search/sapbert/build-job`

Request:

```json
{
  "idempotency_key": "sapbert-athena-20250827-standard-v1",
  "overwrite": false,
  "model_artifact_id": "sapbert-xlmr-merged-v1",
  "scope": {
    "standard_concept": "S",
    "invalid_reason": null
  },
  "batch_size": 32
}
```

Response:

```json
{
  "job_id": "job_sapbert_abc",
  "state": "queued",
  "status_url": "/jobs/job_sapbert_abc"
}
```

### 6.4 `POST /search/concepts`

Request:

```json
{
  "q": "tramadol 50 mg capsule",
  "mode": "hybrid_rrf",
  "limit": 20,
  "filters": {
    "domain_id": ["Drug"],
    "vocabulary_id": ["RxNorm", "RxNorm Extension"],
    "concept_class_id": ["Clinical Drug", "Clinical Drug Comp"]
  },
  "hybrid": {
    "rrf_k": 60,
    "lexical_top_k": 100,
    "sapbert_top_k": 100
  }
}
```

Valid modes:

```text
lexical_tantivy
sapbert_cls
hybrid_rrf
```

Response:

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
    "tantivy_artifact_id": "athena-20250827-tantivy-v1",
    "sapbert_artifact_id": "athena-20250827-sapbert-cls-v1"
  }
}
```

### 6.5 `POST /search/batch`

Request:

```json
{
  "mode": "hybrid_rrf",
  "limit_per_item": 20,
  "items": [
    {
      "id": "src_001",
      "q": "tramadol 50 mg capsule",
      "filters": {
        "domain_id": ["Drug"]
      }
    }
  ]
}
```

Response:

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
          }
        }
      ]
    }
  ]
}
```

### 6.6 `POST /search/explain`

Request:

```json
{
  "q": "tramadol 50 mg capsule",
  "concept_id": 40162522,
  "mode": "hybrid_rrf"
}
```

Response:

```json
{
  "query": "tramadol 50 mg capsule",
  "concept_id": 40162522,
  "explanation": {
    "tantivy": {
      "rank": 1,
      "score": 12.83,
      "matched_fields": ["concept_name", "synonyms"]
    },
    "sapbert": {
      "rank": 4,
      "score": 0.832
    },
    "hybrid_rrf": {
      "score": 0.0318,
      "rrf_k": 60
    }
  }
}
```

## 7. Mapper API details

### 7.1 `GET /mapper/status`

Response:

```json
{
  "status": "ready",
  "api_version": "0.1.0",
  "domain_support": ["Drug"],
  "model": {
    "status": "ready",
    "model_id": "sidataplus/THIRAWAT-SapBERT",
    "projection_dim": 128
  },
  "indexes": {
    "thirawat_doc_embeddings": {
      "status": "ready",
      "artifact_id": "athena-20250827-thirawat-drug-docemb-v1"
    },
    "tachiom": {
      "status": "ready",
      "artifact_id": "athena-20250827-thirawat-drug-tachiom-v1"
    }
  }
}
```

### 7.2 `POST /mapper/thirawat/build-embeddings-job`

Request:

```json
{
  "idempotency_key": "thirawat-docemb-athena-20250827-drug-v1",
  "overwrite": false,
  "domain_id": "Drug",
  "model_artifact_id": "sidataplus-thirawat-sapbert-merged-v1",
  "batch_size": 16,
  "target_scope": {
    "domain_id": ["Drug"],
    "vocabulary_id": ["RxNorm", "RxNorm Extension"]
  }
}
```

Response:

```json
{
  "job_id": "job_thirawat_docemb_abc",
  "state": "queued",
  "status_url": "/jobs/job_thirawat_docemb_abc"
}
```

### 7.3 `POST /mapper/tachiom/build-index-job`

Request:

```json
{
  "idempotency_key": "tachiom-athena-20250827-thirawat-drug-v1",
  "overwrite": false,
  "doc_embedding_artifact_id": "athena-20250827-thirawat-drug-docemb-v1",
  "build_params": {
    "metric": "maxsim",
    "token_aware_clustering": true
  }
}
```

Response:

```json
{
  "job_id": "job_tachiom_abc",
  "state": "queued",
  "status_url": "/jobs/job_tachiom_abc"
}
```

### 7.4 `POST /mapper/drugs/query`

Request:

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
  },
  "normalization": {
    "inn2usan": true
  }
}
```

Valid mapper modes:

```text
hybrid_rrf
thirawat_tachiom
```

`hybrid_rrf` is a fallback mode using `search-api` behavior. `thirawat_tachiom` is the primary Drug mapper.

Response:

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
        "concept_code": "..."
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
    "thirawat_model_id": "sidataplus/THIRAWAT-SapBERT",
    "tachiom_artifact_id": "athena-20250827-thirawat-drug-tachiom-v1"
  }
}
```

### 7.5 `POST /mapper/drugs/batch`

Use only for small batches and tests.

Request:

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

Response:

```json
{
  "mode": "thirawat_tachiom",
  "items": [
    {
      "id": "src_001",
      "candidates": [
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
            "tachiom_maxsim": 0.88,
            "bimaxsim": 0.93,
            "tie_breaker": 0.04,
            "final": 0.93
          }
        }
      ],
      "error": null
    }
  ]
}
```

### 7.6 `POST /mapper/drugs/batch-job`

Use for real project-scale auto-map jobs.

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
  "status_url": "/jobs/job_map_abc"
}
```

### 7.7 `POST /mapper/drugs/explain`

Request:

```json
{
  "source_name": "tramadol hydrochloride 50 mg capsule",
  "source_code": "SRC001",
  "concept_id": 40162522,
  "mode": "thirawat_tachiom"
}
```

Response:

```json
{
  "query": {
    "source_name": "tramadol hydrochloride 50 mg capsule",
    "source_code": "SRC001",
    "query_text": "tramadol hydrochloride 50 mg capsule (SRC001)"
  },
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
    "tachiom_maxsim": 0.88,
    "bimaxsim": 0.93,
    "tie_breaker": 0.04,
    "final": 0.93
  },
  "features": {
    "ingredient_match": true,
    "strength_exact": true,
    "dose_form_match": true,
    "release_match": null,
    "brand_match": null,
    "combination_count_match": true
  },
  "token_debug": {
    "enabled": false,
    "message": "Token-level debug is disabled by default"
  }
}
```

## 8. Jobs API details

See `jobs.md` for full job semantics. Endpoint contract summary:

### 8.1 `GET /jobs/:id`

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

### 8.2 `GET /jobs/:id/events`

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
        "processed": 2400,
        "total": 5000
      },
      "created_at": "2026-05-31T00:10:00Z"
    }
  ]
}
```

### 8.3 `GET /jobs/:id/results`

For small results:

```json
{
  "job_id": "job_map_abc",
  "state": "succeeded",
  "result": {
    "items": []
  }
}
```

For large results:

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

### 8.4 `POST /jobs/:id/cancel`

Response:

```json
{
  "job_id": "job_map_abc",
  "state": "cancelled"
}
```

### 8.5 `POST /jobs/:id/retry`

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
  "status_url": "/jobs/job_map_retry_def"
}
```

## 9. Deferred translation API

Translation is optional and later.

Reasons:

- Core goal is API stability for Rails.
- Translation is not required for the initial Rails review workflow.
- TranslateGemma 4-bit or lower quantized runtime needs a separate benchmark and backend decision.
- Mapper must not depend on translation in v1.

When implemented, it should be a separate `translate-api` module.

```text
GET  /translate/status
POST /translate/text
POST /translate/batch
POST /translate/batch-job
```

Default future model direction:

```text
TranslateGemma 4B
4-bit or lower quantized
GGUF Rust backend allowed
medical_literal prompt mode
number/unit preservation checks
```

## 10. Implementation priority

1. Health endpoints and shared error envelope.
2. Jobs substrate.
3. Catalog standard-only build and lookup.
4. Tantivy lexical search.
5. SapBERT CLS + USearch.
6. THIRAWAT Candle model parity.
7. THIRAWAT Drug document embedding artifacts.
8. Tachiom build and query.
9. External BiMaxSim rerank.
10. Deterministic tie-breaker.
11. Async mapper batch jobs.
12. Rails integration work begins after API spine stabilizes.
