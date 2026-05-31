# `usagi-api` implementation spec v0.1

## 0. Goal

Build a stable Rust-first API platform for Usagi v3:

```text
usagi-api
  = catalog truth
  + lexical search
  + simple semantic search
  + THIRAWAT drug mapper
  + async bulk jobs
```

Rails comes **after** this API is stable. Rails will own projects, imports, review workflow, exports, users, and UI. The API owns vocabulary/search/model/retrieval engines.

No desktop app. No serverless edge. No DuckDB. No LLM reranking. No “just one Python CLI call” quietly wedged into production like a raccoon in the ventilation system.

---

# 1. Final architectural decisions

| Area                | Decision                                            |
| ------------------- | --------------------------------------------------- |
| Repo                | Single monorepo: `usagi-on-the-web`                 |
| Development order   | API first, Rails second                             |
| Deployment          | Docker Compose locally, Kamal on DigitalOcean later |
| Catalog truth       | SQLite                                              |
| Catalog scope       | **Only valid standard concepts from Athena**        |
| DuckDB              | **No DuckDB**                                       |
| Lexical search      | Tantivy                                             |
| Simple dense search | SapBERT CLS + USearch                               |
| Embedding runtime   | Candle-first Rust                                   |
| Mapper model        | `sidataplus/THIRAWAT-SapBERT`                       |
| Mapper retrieval    | Tachiom using MaxSim-style retrieval                |
| Mapper rerank       | External exact BiMaxSim after Tachiom retrieval     |
| Mapper domain       | Drug domain only for now                            |
| Translation         | Optional module after Rails front-end completion    |
| LLM support         | None planned                                        |
| Queue               | SQLite-backed API jobs                              |
| Rails coupling      | Rails consumes stable API endpoints later           |

THIRAWAT-SapBERT is a PyLate ColBERT-style model based on SapBERT-XLMR; the model card says it maps text to sequences of 128-dimensional vectors, uses query/document length 96, and uses MaxSim as its similarity function. ([Hugging Face][1]) Tachiom is suitable for the retrieval layer because it is a Rust/Python late-interaction multi-vector retrieval structure, and its quick-start takes token vectors, token IDs, and document lengths, then returns scores and document IDs from `batch_search`. ([GitHub][2])

---

# 2. Repository layout

```text
usagi-on-the-web/
  services/
    catalog-api/
    search-api/
    mapper-api/
    api-worker/
    # translate-api later, optional module

  crates/
    usagi-contracts/
    usagi-catalog/
    usagi-search/
    usagi-embed/
    usagi-thirawat/
    usagi-tachiom/
    usagi-jobs/
    usagi-artifacts/
    usagi-common/

  fixtures/
    athena-mini/
    source-terms/
    thirawat-golden/
    search-golden/

  infra/
    docker/
      docker-compose.api.yml
    kamal/
      # later

  apps/
    web/
      # Rails later

  docs/
    api/
      implementation-spec.md
      endpoints.md
      artifacts.md
      jobs.md
      rails-integration-contract.md
```

## Crate responsibilities

| Crate             | Responsibility                                                         |
| ----------------- | ---------------------------------------------------------------------- |
| `usagi-contracts` | Shared request/response DTOs                                           |
| `usagi-catalog`   | Athena parsing, SQLite schema, concept lookup                          |
| `usagi-search`    | Tantivy + USearch index/search logic                                   |
| `usagi-embed`     | Candle tokenizer/model loading, SapBERT CLS, THIRAWAT token embeddings |
| `usagi-thirawat`  | Drug normalization, MaxSim/BiMaxSim, deterministic tie-breaker         |
| `usagi-tachiom`   | Tachiom artifact build/load/query adapter                              |
| `usagi-jobs`      | SQLite job queue, job state, events, item results                      |
| `usagi-artifacts` | Manifest, checksum, artifact validation                                |
| `usagi-common`    | Error envelope, request IDs, logging, config                           |

---

# 3. Service topology

```text
┌────────────────────────────────────────────────────┐
│ catalog-api                                         │
│ SQLite standard OMOP concept catalog                │
└──────────────┬─────────────────────────────────────┘
               │
               v
┌────────────────────────────────────────────────────┐
│ search-api                                          │
│ Tantivy lexical + USearch SapBERT CLS               │
└──────────────┬─────────────────────────────────────┘
               │
               v
┌────────────────────────────────────────────────────┐
│ mapper-api                                          │
│ THIRAWAT-SapBERT + Tachiom + BiMaxSim + tie-breaker │
└────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────┐
│ api-worker                                          │
│ Async build/index/embed/map jobs                    │
└────────────────────────────────────────────────────┘
```

Optional later:

```text
translate-api
  TranslateGemma 4-bit or lower quantized translation
  Developed after Rails front-end completion
```

---

# 4. Runtime artifacts

```text
data/
  catalog/
    catalog.sqlite
    manifest.json

  search/
    tantivy/
      index/
      manifest.json

    sapbert/
      sapbert_cls.usearch
      concept_ids.arrow
      manifest.json

  mapper/
    thirawat-drug/
      doc_embeddings/
        token_vectors.npy
        token_ids.npy
        doclens.npy
        doc_ids.arrow
        manifest.json

      tachiom/
        index.bin
        manifest.json

      concept_metadata.sqlite

  models/
    sapbert/
      config.json
      tokenizer.json
      model.safetensors
      manifest.json

    thirawat-sapbert/
      config.json
      tokenizer.json
      model.safetensors
      colbert_projection.safetensors
      manifest.json

  jobs/
    jobs.sqlite
```

Every artifact is derived from `catalog.sqlite` or model files. If derived artifacts are deleted or invalid, rebuild them. If the catalog is wrong, everything downstream is wrong with a confident little smile.

---

# 5. Artifact manifest contract

Every artifact gets a `manifest.json`.

```json
{
  "artifact_id": "athena-20250827-standard-v1",
  "artifact_kind": "catalog.sqlite",
  "schema_version": "usagi-catalog-v1",
  "api_version": "0.1.0",
  "created_at": "2026-05-31T00:00:00Z",
  "vocabulary": {
    "source": "ATHENA",
    "version": "20250827",
    "scope": {
      "standard_concept": "S",
      "invalid_reason": null
    }
  },
  "inputs": [
    {
      "path": "CONCEPT.csv",
      "sha256": "..."
    }
  ],
  "outputs": [
    {
      "path": "catalog.sqlite",
      "sha256": "..."
    }
  ]
}
```

## Required manifest fields

| Field                |            Required | Why                                     |
| -------------------- | ------------------: | --------------------------------------- |
| `artifact_id`        |                 yes | stable reference                        |
| `artifact_kind`      |                 yes | catalog/search/sapbert/thirawat/tachiom |
| `schema_version`     |                 yes | compatibility                           |
| `api_version`        |                 yes | reader compatibility                    |
| `created_at`         |                 yes | provenance                              |
| `vocabulary.version` |                 yes | Athena traceability                     |
| `scope`              |                 yes | standard-only guarantee                 |
| `inputs[].sha256`    |                 yes | reproducibility                         |
| `outputs[].sha256`   |                 yes | corruption detection                    |
| `model.model_id`     | for model artifacts | prevents silent model drift             |
| `model.model_hash`   | for model artifacts | because model IDs are not enough        |

---

# 6. Catalog API

## Scope

`catalog-api` stores only:

```sql
standard_concept = 'S'
AND invalid_reason IS NULL
```

This is deliberately narrower than full OMOP. It supports mapping **to** standard concepts, not training from non-standard concepts or browsing full ATHENA. Tiny mercy: the catalog will be smaller, clearer, and less tempted to recommend non-standard junk.

## Tables

```sql
concept
  concept_id integer primary key
  concept_name text not null
  domain_id text not null
  vocabulary_id text not null
  concept_class_id text not null
  standard_concept text not null
  concept_code text not null
  valid_start_date text
  valid_end_date text
  invalid_reason text

concept_synonym
  concept_id integer not null
  concept_synonym_name text not null
  language_concept_id integer

concept_relationship
  concept_id_1 integer not null
  concept_id_2 integer not null
  relationship_id text not null
  valid_start_date text
  valid_end_date text
  invalid_reason text

concept_ancestor
  ancestor_concept_id integer not null
  descendant_concept_id integer not null
  min_levels_of_separation integer
  max_levels_of_separation integer

vocabulary
domain
concept_class
relationship
catalog_metadata
```

## Important ingestion rule

When loading relationship tables, retain only relationships where both sides exist in the standard-concept catalog, unless a relationship is needed for standard concept hierarchy integrity. This avoids accidentally reintroducing non-standard concepts through the side door, because apparently vocabularies also enjoy smuggling.

## Endpoints

```text
GET  /catalog/health
GET  /catalog/status
POST /catalog/build-job

GET  /catalog/concepts/:concept_id
POST /catalog/concepts/batch

GET  /catalog/concepts/:concept_id/ancestors
GET  /catalog/concepts/:concept_id/descendants
GET  /catalog/concepts/:concept_id/relationships

GET  /catalog/domains
GET  /catalog/vocabularies
GET  /catalog/concept-classes
```

## Catalog status response

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

---

# 7. Search API

## Search modes

```text
lexical_tantivy
sapbert_cls
hybrid_rrf
```

## Components

```text
Tantivy
  lexical BM25 search over standard concepts

SapBERT CLS + USearch
  one vector per concept
  simple dense retrieval baseline

Hybrid RRF
  rank fusion of Tantivy + SapBERT
```

SapBERT CLS is for general semantic retrieval. It is not the THIRAWAT mapper. That distinction matters, because one pooled vector per concept is useful but not enough to reliably resolve drug strength/form/release-level ambiguity. The THIRAWAT paper explicitly notes pooled bi-encoder similarity can be insufficient for clinically similar drug concepts, especially where strength, form/route, release, or combination products differ. 

## Endpoints

```text
GET  /search/health
GET  /search/status

POST /search/tantivy/build-job
POST /search/sapbert/build-job

POST /search/concepts
POST /search/batch
POST /search/explain
```

## `/search/concepts` request

```json
{
  "q": "tramadol 50 mg capsule",
  "mode": "hybrid_rrf",
  "limit": 20,
  "filters": {
    "domain_id": ["Drug"],
    "vocabulary_id": ["RxNorm", "RxNorm Extension"],
    "concept_class_id": ["Clinical Drug", "Clinical Drug Comp"],
    "standard_concept": "S"
  },
  "hybrid": {
    "rrf_k": 60,
    "lexical_top_k": 100,
    "sapbert_top_k": 100
  }
}
```

## Response

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

## Hybrid RRF rule

```text
score(candidate) = Σ 1 / (rrf_k + rank_i)
```

Default:

```text
rrf_k = 60
lexical_top_k = 100
sapbert_top_k = 100
final_limit = 20
```

No raw BM25 + cosine score blending. That road leads to fake precision and hurt feelings.

---

# 8. Embedding with Candle

## `usagi-embed` responsibilities

```text
- load tokenizer.json
- load model.safetensors
- choose device
- batch tokenization
- pad/truncate
- build attention masks
- run Candle forward pass
- extract CLS embeddings for SapBERT
- extract token embeddings for THIRAWAT
- L2 normalize outputs
- run parity tests against Python references
```

## Device config

```toml
[embed]
device = "cpu"       # cpu | cuda:0 later
batch_size = 32
max_length = 96
```

## SapBERT CLS output

```rust
pub struct ClsEmbedding {
    pub text: String,
    pub vector: Vec<f32>
}
```

## THIRAWAT token output

```rust
pub struct TokenEmbedding {
    pub text: String,
    pub token_ids: Vec<i64>,
    pub vectors: Vec<Vec<f32>>,
    pub attention_mask: Vec<bool>
}
```

## Parity requirement

Before using Candle embeddings in indexes:

| Output                 | Required                                           |
| ---------------------- | -------------------------------------------------- |
| Token IDs              | exact match with Python reference                  |
| Attention mask         | exact match                                        |
| SapBERT CLS vector     | cosine ≥ 0.999 against Python reference            |
| THIRAWAT token vectors | mean token cosine ≥ 0.999 against Python reference |
| Sequence truncation    | identical behavior                                 |

If parity fails, the endpoint does not “mostly work.” It fails. Scientific software deserves fewer vibes and more numbers.

---

# 9. THIRAWAT mapper API

## Scope

THIRAWAT mapper supports:

```text
domain_id = Drug
```

Only Drug for now. Conditions, procedures, measurements, and other domains are explicitly future expansion.

The THIRAWAT paper focused on drug concept mapping and identifies drug-specific failure modes involving strength, form, route, release, brand, and combinations; future work in the paper discusses possible expansion to non-drug domains, but v1 here stays Drug-only because we are not trying to rebuild the universe before Rails exists. 

## Pipeline

```text
source drug string
  -> normalize
  -> Candle THIRAWAT query token embeddings
  -> Tachiom MaxSim-style retrieval
  -> hydrate candidate concepts from catalog.sqlite
  -> external exact BiMaxSim over top-N
  -> deterministic drug tie-breaker
  -> candidate JSON response
```

## Important scoring decision

Tachiom is used for **retrieval by MaxSim-style late-interaction score**.

Then `mapper-api` performs external exact BiMaxSim reranking.

```text
Tachiom score
  = fast retrieval score

BiMaxSim score
  = exact external reranking score

Tie-breaker
  = deterministic near-tie resolver
```

The paper defines BiMaxSim as a symmetric token-coverage score that rewards candidate coverage of query content and down-weights candidates introducing unsupported tokens; it applies BiMaxSim at inference during top-k reranking. 

## Endpoints

```text
GET  /mapper/health
GET  /mapper/status

POST /mapper/thirawat/build-embeddings-job
POST /mapper/tachiom/build-index-job

POST /mapper/drugs/query
POST /mapper/drugs/batch
POST /mapper/drugs/batch-job
POST /mapper/drugs/explain
```

## `/mapper/drugs/query` request

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

## Response

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
    "model_artifact_id": "sidataplus/THIRAWAT-SapBERT",
    "index_artifact_id": "athena-20250827-thirawat-drug-tachiom-v1"
  }
}
```

## Tie-break defaults

```text
epsilon = 0.01
top_n = 100
```

These match the THIRAWAT paper’s conservative near-tie defaults: it used `ε = 0.01` after observing top-200 score gaps up to 0.007, and it used `N = 100` after a small sweep over 20, 100, and 200. 

## No LLM support

No LLM reordering is planned. The paper evaluated LLM reordering only as an optional post hoc comparison and explicitly notes it was not part of the primary pipeline because it added latency, cost, and non-determinism.  This API will choose determinism and auditability over chatbot roulette. Refreshing, almost.

---

# 10. THIRAWAT model artifacts

The model card states the THIRAWAT-SapBERT architecture includes a transformer with PEFT feature-extraction architecture plus a dense projection from 768 to 128 without bias. ([Hugging Face][1])

## Required artifact layout

```text
data/models/thirawat-sapbert/
  config.json
  tokenizer.json
  tokenizer_config.json
  special_tokens_map.json
  model.safetensors
  colbert_projection.safetensors
  manifest.json
```

## Manifest

```json
{
  "model_id": "sidataplus/THIRAWAT-SapBERT",
  "base_model": "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR",
  "architecture": "pylate_colbert",
  "encoder_family": "xlm-roberta",
  "query_length": 96,
  "document_length": 96,
  "hidden_dim": 768,
  "projection_dim": 128,
  "similarity": "maxsim",
  "projection": {
    "in_features": 768,
    "out_features": 128,
    "bias": false
  },
  "peft": {
    "merged": true
  },
  "sha256": {
    "model.safetensors": "...",
    "colbert_projection.safetensors": "..."
  }
}
```

## Runtime rule

Require merged weights if possible. Do not implement PEFT/LoRA merging in the hot Rust inference path. That would be a brave little footgun.

---

# 11. Tachiom artifacts

Tachiom build inputs:

```text
vectors.npy     [N_tokens, dim]
token_ids.npy   [N_tokens]
doclens.npy     [N_docs]
doc_ids.arrow   [N_docs]
```

The Tachiom README shows this exact build/search shape: token vectors, token IDs, and document lengths are used to build an index, and `batch_search` receives query token vectors and returns scores plus document IDs. ([GitHub][2])

## Artifact layout

```text
data/mapper/thirawat-drug/
  doc_embeddings/
    vectors.npy
    token_ids.npy
    doclens.npy
    doc_ids.arrow
    manifest.json

  tachiom/
    index.bin
    manifest.json

  concept_metadata.sqlite
```

## Build pipeline

```text
catalog.sqlite
  -> select standard valid Drug concepts
  -> build THIRAWAT document text
  -> Candle encode document token embeddings
  -> write vectors/token_ids/doclens/doc_ids
  -> build Tachiom index
  -> write manifest
```

## Query pipeline

```text
source term
  -> Candle encode query token embeddings
  -> Tachiom batch_search by MaxSim-style retrieval
  -> retrieve top candidate document IDs
  -> hydrate concepts
  -> exact BiMaxSim rerank
  -> deterministic tie-break
```

---

# 12. Job system

## Job store

```text
data/jobs/jobs.sqlite
```

## Tables

```sql
jobs
  id text primary key
  kind text not null
  queue text not null
  state text not null
  stage text
  idempotency_key text unique
  input_json text not null
  result_json text
  error_json text
  artifact_path text
  total integer default 0
  processed integer default 0
  failed integer default 0
  created_at text not null
  started_at text
  finished_at text
  updated_at text not null

job_events
  id text primary key
  job_id text not null
  seq integer not null
  level text not null
  message text not null
  payload_json text
  created_at text not null

job_items
  id text primary key
  job_id text not null
  item_key text not null
  state text not null
  input_json text
  result_json text
  error_json text
  attempt_count integer default 0
  updated_at text not null
```

## Job states

```text
queued
running
succeeded
succeeded_with_errors
failed
cancelled
```

## Async-only endpoints

All build and bulk endpoints are async-only:

```text
/catalog/build-job
/search/tantivy/build-job
/search/sapbert/build-job
/mapper/thirawat/build-embeddings-job
/mapper/tachiom/build-index-job
/mapper/drugs/batch-job
```

Small query endpoints may be sync:

```text
/search/concepts
/mapper/drugs/query
/mapper/drugs/batch
```

## Job endpoints

```text
GET  /jobs/:id
GET  /jobs/:id/events
GET  /jobs/:id/results
POST /jobs/:id/cancel
POST /jobs/:id/retry
```

## Job status response

```json
{
  "id": "job_abc",
  "kind": "mapper_drugs_batch",
  "state": "running",
  "stage": "bimaxsim_reranking",
  "processed": 2400,
  "total": 5000,
  "failed": 3,
  "message": "Reranking candidates",
  "created_at": "2026-05-31T00:00:00Z",
  "updated_at": "2026-05-31T00:10:00Z"
}
```

---

# 13. Idempotency

Required for every build and bulk job.

```text
idempotency_key =
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

If the same key is submitted again:

```text
return existing job
```

No duplicate index builds. No accidental six-hour reruns because someone double-clicked a button like it owed them money.

---

# 14. Worker design

Use a single worker binary first:

```text
usagi-worker --queues index,embed,map
```

Later split if needed.

## Queues

| Queue     | Jobs                                              |
| --------- | ------------------------------------------------- |
| `catalog` | `catalog_build`                                   |
| `index`   | `tantivy_build`, `sapbert_build`, `tachiom_build` |
| `embed`   | `sapbert_embed`, `thirawat_doc_embed`             |
| `map`     | `mapper_drugs_batch`                              |

## Concurrency defaults

| Queue     |        Concurrency | Reason                  |
| --------- | -----------------: | ----------------------- |
| `catalog` |                  1 | catalog file safety     |
| `index`   |                  1 | artifact safety         |
| `embed`   | 1 per model/device | model memory            |
| `map`     |                1-2 | Tachiom + BiMaxSim cost |

## Job stages for THIRAWAT batch mapping

```text
queued
normalizing
embedding_queries
tachiom_retrieval
bimaxsim_reranking
deterministic_tiebreak
writing_results
succeeded
```

---

# 15. Error model

All services return the same envelope.

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

## Standard error codes

```text
BAD_REQUEST
NOT_FOUND
CATALOG_NOT_READY
INDEX_NOT_READY
MODEL_NOT_READY
INCOMPATIBLE_ARTIFACT
JOB_NOT_FOUND
JOB_CANCELLED
JOB_FAILED
EMBEDDING_FAILED
TACHIOM_FAILED
INTERNAL_ERROR
```

---

# 16. Observability

## Required structured log fields

```text
request_id
job_id
service
endpoint
stage
artifact_id
catalog_artifact_id
model_artifact_id
index_artifact_id
duration_ms
items_processed
error_code
```

## Metrics

| Metric                           | Why                      |
| -------------------------------- | ------------------------ |
| `catalog_build_duration_seconds` | catalog ingestion health |
| `tantivy_search_latency_ms`      | UI search responsiveness |
| `sapbert_embedding_latency_ms`   | model overhead           |
| `usearch_query_latency_ms`       | dense retrieval          |
| `tachiom_query_latency_ms`       | mapper retrieval         |
| `bimaxsim_rerank_latency_ms`     | reranker bottleneck      |
| `mapper_items_per_second`        | batch throughput         |
| `job_failure_count`              | operational sanity       |
| `artifact_size_bytes`            | pack/storage planning    |

The THIRAWAT paper’s runtime profiling shows reranking dominates CPU latency and GPU reduces p50 end-to-end latency from roughly 7.3-7.7 seconds to 1.5-1.6 seconds, so instrumenting rerank cost separately is not optional if we enjoy knowing why things are slow. 

---

# 17. API endpoint summary

## Catalog

```text
GET  /catalog/health
GET  /catalog/status
POST /catalog/build-job
GET  /catalog/concepts/:concept_id
POST /catalog/concepts/batch
GET  /catalog/concepts/:concept_id/ancestors
GET  /catalog/concepts/:concept_id/descendants
GET  /catalog/concepts/:concept_id/relationships
GET  /catalog/domains
GET  /catalog/vocabularies
GET  /catalog/concept-classes
```

## Search

```text
GET  /search/health
GET  /search/status
POST /search/tantivy/build-job
POST /search/sapbert/build-job
POST /search/concepts
POST /search/batch
POST /search/explain
```

## Mapper

```text
GET  /mapper/health
GET  /mapper/status
POST /mapper/thirawat/build-embeddings-job
POST /mapper/tachiom/build-index-job
POST /mapper/drugs/query
POST /mapper/drugs/batch
POST /mapper/drugs/batch-job
POST /mapper/drugs/explain
```

## Jobs

```text
GET  /jobs/:id
GET  /jobs/:id/events
GET  /jobs/:id/results
POST /jobs/:id/cancel
POST /jobs/:id/retry
```

## Translation later

```text
GET  /translate/status
POST /translate/text
POST /translate/batch
POST /translate/batch-job
```

Not in API v1 stabilization. Park the translator in the later pile where it can stare wistfully at Q4 quantization.

---

# 18. Docker Compose API-only target

```yaml
services:
  catalog-api:
    build:
      context: .
      dockerfile: services/catalog-api/Dockerfile
    ports:
      - "8788:8788"
    volumes:
      - ./data:/data
      - ./fixtures:/fixtures:ro
    environment:
      CATALOG_DB_PATH: /data/catalog/catalog.sqlite
      JOBS_DB_PATH: /data/jobs/jobs.sqlite

  search-api:
    build:
      context: .
      dockerfile: services/search-api/Dockerfile
    ports:
      - "8789:8789"
    volumes:
      - ./data:/data
    environment:
      CATALOG_DB_PATH: /data/catalog/catalog.sqlite
      TANTIVY_INDEX_DIR: /data/search/tantivy/index
      SAPBERT_INDEX_DIR: /data/search/sapbert
      SAPBERT_MODEL_DIR: /data/models/sapbert

  mapper-api:
    build:
      context: .
      dockerfile: services/mapper-api/Dockerfile
    ports:
      - "8790:8790"
    volumes:
      - ./data:/data
    environment:
      CATALOG_DB_PATH: /data/catalog/catalog.sqlite
      THIRAWAT_MODEL_DIR: /data/models/thirawat-sapbert
      THIRAWAT_ARTIFACT_DIR: /data/mapper/thirawat-drug
      TACHIOM_INDEX_DIR: /data/mapper/thirawat-drug/tachiom

  api-worker:
    build:
      context: .
      dockerfile: services/api-worker/Dockerfile
    volumes:
      - ./data:/data
      - ./fixtures:/fixtures:ro
    environment:
      JOBS_DB_PATH: /data/jobs/jobs.sqlite
      CATALOG_DB_PATH: /data/catalog/catalog.sqlite
      WORKER_QUEUES: catalog,index,embed,map
      EMBED_DEVICE: cpu
```

---

# 19. First implementation milestones

## M0: Workspace and contracts

Deliver:

```text
- Rust workspace
- DTO structs in usagi-contracts
- shared error envelope
- service skeletons
- Docker Compose boots health endpoints
```

Acceptance:

```text
curl /catalog/health
curl /search/health
curl /mapper/health
```

---

## M1: Jobs substrate

Deliver:

```text
- jobs.sqlite schema
- job creation
- job polling
- job events
- worker loop
```

Acceptance:

```text
POST dummy job
worker completes it
GET /jobs/:id returns succeeded
```

---

## M2: Catalog API

Deliver:

```text
- Athena parser
- standard valid concept filtering
- catalog.sqlite builder
- manifest writer
- concept lookup
- hierarchy endpoints
```

Acceptance:

```text
catalog_build job from athena-mini
standard concept lookup works
non-standard concepts excluded
manifest validates
```

Complexity:

```text
Athena ingest: O(C + S + R + A)
SQLite indexes: practical O(N log N)
```

---

## M3: Tantivy search

Deliver:

```text
- Tantivy schema
- index build job
- lexical search
- filters
- explain endpoint
```

Acceptance:

```text
search "tramadol 50 mg capsule"
returns expected fixture concept
```

Complexity:

```text
Index build: O(C * L)
Query: roughly O(sum(postings) + K log K)
```

---

## M4: SapBERT CLS + USearch

Deliver:

```text
- Candle SapBERT CLS encoder
- Python reference parity fixture
- USearch index build job
- sapbert_cls search mode
- hybrid_rrf search mode
```

Acceptance:

```text
CLS parity cosine >= 0.999
sapbert search returns expected concept
hybrid RRF returns stable golden result
```

---

## M5: THIRAWAT model probe

Deliver:

```text
- load THIRAWAT-SapBERT tokenizer/model/projection
- encode query/doc token vectors
- Python parity fixture
```

Acceptance:

```text
token IDs exact match
token vectors mean cosine >= 0.999
output dim = 128
max length = 96
```

---

## M6: THIRAWAT token artifact build

Deliver:

```text
- select Drug standard concepts
- build document texts
- encode THIRAWAT doc token vectors
- write token_vectors/token_ids/doclens/doc_ids
- write manifest
```

Acceptance:

```text
artifact validates
doc_ids map to Drug standard concepts only
```

---

## M7: Tachiom build/query

Deliver:

```text
- Tachiom index builder
- Tachiom index loader
- MaxSim-style retrieval
- concept hydration
```

Acceptance:

```text
known drug query retrieves expected candidate in top K
```

---

## M8: External BiMaxSim + deterministic tie-breaker

Deliver:

```text
- exact MaxSim
- exact BiMaxSim
- strength/form/route/release/brand features
- near-tie reorder
- explain output
```

Acceptance:

```text
BiMaxSim golden tests pass
tie-breaker golden tests pass
mapper query returns expected rank order
```

Complexity:

```text
External BiMaxSim:
  O(K * |Q| * |D| * dim)

With K = rerank_top_n,
     |Q|, |D| <= 96,
     dim = 128
```

---

## M9: Mapper batch jobs

Deliver:

```text
- /mapper/drugs/batch-job
- job_items result storage
- partial failure support
- result artifact export
```

Acceptance:

```text
500-source fixture maps through async job
failed item does not fail whole job
GET /jobs/:id/results returns candidate JSONL
```

---

# 20. Rails integration contract

Rails later will call only stable product-grade endpoints:

```text
GET  /catalog/status
GET  /search/status
GET  /mapper/status

POST /search/concepts
POST /mapper/drugs/query
POST /mapper/drugs/batch-job

GET  /jobs/:id
GET  /jobs/:id/events
GET  /jobs/:id/results
```

Rails will persist:

```text
projects
source_terms
mappings
mapping_candidates
engine_jobs
exports
audit_events
```

Rails will not persist:

```text
embeddings
Tantivy internals
USearch vectors
Tachiom internals
model weights
```

The API stays the engine room. Rails gets the steering wheel. Nobody lets the steering wheel edit the token-vector index. 🚗

---

# 21. Testing gates

| Gate               | Required tests                          |
| ------------------ | --------------------------------------- |
| Catalog            | Athena mini → standard-only SQLite      |
| Search lexical     | known query → known concept             |
| SapBERT            | Candle vs Python CLS parity             |
| Hybrid             | deterministic RRF golden result         |
| THIRAWAT model     | Candle vs Python token parity           |
| Tachiom            | build/load/query smoke                  |
| BiMaxSim           | exact score fixtures                    |
| Tie-breaker        | feature extraction and reorder fixtures |
| Mapper             | end-to-end drug query golden            |
| Jobs               | cancel/retry/resume/partial failure     |
| Artifact manifests | reject mismatch/corruption              |
| Docker             | all services boot and pass smoke script |

---

# 22. Smoke script target

```bash
#!/usr/bin/env bash
set -euo pipefail

docker compose -f infra/docker/docker-compose.api.yml up -d --build

curl -fsS localhost:8788/catalog/health
curl -fsS localhost:8789/search/health
curl -fsS localhost:8790/mapper/health

curl -fsS -X POST localhost:8788/catalog/build-job \
  -H 'content-type: application/json' \
  -d '{"athena_dir":"/fixtures/athena-mini"}'

# poll job...

curl -fsS -X POST localhost:8789/search/tantivy/build-job \
  -H 'content-type: application/json' \
  -d '{}'

# poll job...

curl -fsS -X POST localhost:8789/search/concepts \
  -H 'content-type: application/json' \
  -d '{"q":"tramadol 50 mg capsule","mode":"lexical_tantivy","limit":5}'
```

Later smoke adds SapBERT, THIRAWAT, and Tachiom.

---

# 23. Non-goals for API stabilization

Explicitly **not** in API stabilization:

```text
- Rails UI
- TranslateGemma implementation
- LLM reranking
- non-drug THIRAWAT mapping
- cloud/serverless deployment
- desktop app
- full non-standard Athena source mapping support
- live collaboration
```

This is not “lack of ambition.” This is scope control, that mythical beast developers keep describing but rarely feeding.

---

# Final implementation stance

Build `usagi-api` in this order:

```text
1. jobs substrate
2. standard-only SQLite catalog
3. Tantivy lexical search
4. Candle SapBERT CLS + USearch
5. THIRAWAT model parity
6. THIRAWAT Drug token artifacts
7. Tachiom MaxSim retrieval
8. external BiMaxSim reranking
9. deterministic drug tie-breaker
10. async batch mapping
```

Once this is stable, Rails development becomes straightforward:

```text
Rails imports source terms
Rails calls mapper batch job
Rails polls job status
Rails persists candidates
Rails renders review UI
Rails exports mappings
```

That’s the spine. Build this first and Rails won’t have to carry the entire vocabulary/model circus on its back like some tragic circus camel.

[1]: https://huggingface.co/sidataplus/THIRAWAT-SapBERT "sidataplus/THIRAWAT-SapBERT · Hugging Face"
[2]: https://github.com/TusKANNy/tachiom "GitHub - TusKANNy/tachiom: Official repository of S. Martinico, F. M. Nardini, C. Rulli, and R. Venturini. \"Efficient Multivector Retrieval with Token-Aware Clustering and Hierarchical Indexing\" Short Paper @ ACM SIGIR 2026. · GitHub"
