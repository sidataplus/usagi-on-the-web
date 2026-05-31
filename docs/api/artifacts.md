# usagi-api artifacts

Status: draft v0.1  
Audience: API implementers, deployment maintainers, Rails integrators  
Scope: runtime and build artifacts for catalog, search, mapper, and jobs

## 1. Artifact principles

`usagi-api` is artifact-driven.

```text
catalog.sqlite
  = runtime vocabulary truth

Tantivy index
  = derived lexical search artifact

USearch index
  = derived SapBERT CLS dense search artifact

THIRAWAT document token embeddings
  = derived Drug-domain mapper embedding artifact

Tachiom index
  = derived late-interaction Drug mapper retrieval artifact

jobs.sqlite
  = API job state, not product workflow state
```

Rules:

1. SQLite is the catalog truth.
2. DuckDB is not used.
3. Catalog contains only valid standard Athena concepts.
4. All indexes are derived and rebuildable.
5. Every artifact has a manifest.
6. Every manifest has checksums.
7. Artifact compatibility is checked before use.
8. Rails later persists product workflow state, not model/index internals.

## 2. Directory layout

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
      tokenizer_config.json
      special_tokens_map.json
      model.safetensors
      colbert_projection.safetensors
      manifest.json

  jobs/
    jobs.sqlite
    results/
      job_<id>/
        results.jsonl
        manifest.json
```

## 3. Artifact dependency graph

```text
ATHENA files
  -> catalog.sqlite
      -> Tantivy index
      -> SapBERT CLS embeddings
          -> USearch index
      -> THIRAWAT Drug document token embeddings
          -> Tachiom index
```

More explicit:

```text
CONCEPT.csv
CONCEPT_SYNONYM.csv
CONCEPT_RELATIONSHIP.csv
CONCEPT_ANCESTOR.csv
VOCABULARY.csv
DOMAIN.csv
CONCEPT_CLASS.csv
RELATIONSHIP.csv
  -> catalog.sqlite

catalog.sqlite
  -> search/tantivy/index/

catalog.sqlite + SapBERT model
  -> search/sapbert/sapbert_cls.usearch

catalog.sqlite + THIRAWAT-SapBERT model
  -> mapper/thirawat-drug/doc_embeddings/

doc_embeddings/
  -> mapper/thirawat-drug/tachiom/index.bin
```

## 4. Common manifest schema

Every artifact has a manifest with this common header.

```json
{
  "artifact_id": "athena-20250827-standard-v1",
  "artifact_kind": "catalog.sqlite",
  "schema_version": "usagi-catalog-v1",
  "api_version": "0.1.0",
  "created_at": "2026-05-31T00:00:00Z",
  "created_by": {
    "service": "catalog-api",
    "job_id": "job_catalog_abc"
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

### Required fields

| Field | Required | Description |
|---|---:|---|
| `artifact_id` | yes | Unique artifact identifier |
| `artifact_kind` | yes | Artifact type |
| `schema_version` | yes | Reader schema compatibility |
| `api_version` | yes | Producer API version |
| `created_at` | yes | ISO timestamp |
| `created_by.service` | yes | Producing service |
| `created_by.job_id` | yes for jobs | Producing job |
| `inputs[].sha256` | yes | Input integrity |
| `outputs[].sha256` | yes | Output integrity |

## 5. Catalog artifact

### 5.1 Scope

The catalog contains only:

```sql
standard_concept = 'S'
AND invalid_reason IS NULL
```

This scope applies at ingestion time, not merely at query time.

### 5.2 Catalog manifest

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
  "counts": {
    "concepts": 1234567,
    "synonyms": 2345678,
    "relationships": 3456789,
    "ancestors": 4567890
  },
  "inputs": [
    {
      "path": "CONCEPT.csv",
      "sha256": "..."
    },
    {
      "path": "CONCEPT_SYNONYM.csv",
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

### 5.3 SQLite schema

```sql
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
```

### 5.4 Required indexes

```sql
CREATE INDEX idx_concept_name
  ON concept(concept_name);

CREATE INDEX idx_concept_domain_vocab_class
  ON concept(domain_id, vocabulary_id, concept_class_id);

CREATE INDEX idx_concept_code_vocab
  ON concept(vocabulary_id, concept_code);

CREATE INDEX idx_synonym_concept
  ON concept_synonym(concept_id);

CREATE INDEX idx_synonym_name
  ON concept_synonym(concept_synonym_name);

CREATE INDEX idx_rel_1
  ON concept_relationship(concept_id_1, relationship_id);

CREATE INDEX idx_rel_2
  ON concept_relationship(concept_id_2, relationship_id);

CREATE INDEX idx_ancestor_desc
  ON concept_ancestor(descendant_concept_id);

CREATE INDEX idx_ancestor_anc
  ON concept_ancestor(ancestor_concept_id);
```

### 5.5 Validation rules

A catalog artifact is valid only if:

1. `catalog.sqlite` exists.
2. `manifest.json` exists.
3. Output SHA256 matches.
4. `concept` table is non-empty.
5. All rows in `concept` have `standard_concept = 'S'`.
6. All rows in `concept` have `invalid_reason IS NULL`.
7. `catalog_metadata.status = ready`.
8. Vocabulary version is present or explicitly set to `unknown`.

## 6. Tantivy lexical index artifact

### 6.1 Purpose

The Tantivy index supports:

```text
lexical_tantivy search
typeahead-style concept search
search explanation
manual Rails concept search later
```

### 6.2 Indexed fields

| Field | Type | Purpose |
|---|---|---|
| `concept_id` | u64 fast + stored | hydration |
| `concept_name` | text + stored | primary lexical field |
| `concept_name_norm` | text | normalized matching |
| `concept_code` | text + stored | code lookup |
| `domain_id` | facet/fast/stored | filters |
| `vocabulary_id` | facet/fast/stored | filters |
| `concept_class_id` | facet/fast/stored | filters |
| `standard_concept` | fast/stored | invariant/filter |
| `synonyms` | text | synonym matching |
| `search_blob` | text | combined text |

### 6.3 Manifest

```json
{
  "artifact_id": "athena-20250827-tantivy-v1",
  "artifact_kind": "tantivy-index",
  "schema_version": "usagi-tantivy-v1",
  "api_version": "0.1.0",
  "created_at": "2026-05-31T00:00:00Z",
  "catalog": {
    "artifact_id": "athena-20250827-standard-v1",
    "sha256": "..."
  },
  "index": {
    "document_count": 1234567,
    "fields": [
      "concept_id",
      "concept_name",
      "concept_name_norm",
      "concept_code",
      "domain_id",
      "vocabulary_id",
      "concept_class_id",
      "synonyms",
      "search_blob"
    ]
  },
  "outputs": [
    {
      "path": "index/",
      "sha256_tree": "..."
    }
  ]
}
```

### 6.4 Rebuild triggers

Rebuild the Tantivy index when:

- catalog artifact changes
- Tantivy schema version changes
- tokenizer/analyzer config changes
- concept search text generation changes

## 7. SapBERT CLS + USearch artifact

### 7.1 Purpose

SapBERT CLS supports simple semantic retrieval and hybrid RRF search.

It is not the final THIRAWAT mapper.

### 7.2 Model

Default model:

```text
cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR
```

Pooling:

```text
CLS
```

Vector normalization:

```text
L2 normalize before indexing and query
```

### 7.3 Files

```text
search/sapbert/
  sapbert_cls.usearch
  concept_ids.arrow
  manifest.json
```

### 7.4 Manifest

```json
{
  "artifact_id": "athena-20250827-sapbert-cls-v1",
  "artifact_kind": "usearch-sapbert-cls",
  "schema_version": "usagi-sapbert-cls-v1",
  "api_version": "0.1.0",
  "created_at": "2026-05-31T00:00:00Z",
  "catalog": {
    "artifact_id": "athena-20250827-standard-v1",
    "sha256": "..."
  },
  "model": {
    "model_id": "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR",
    "model_artifact_id": "sapbert-xlmr-merged-v1",
    "pooling": "cls",
    "dimension": 768,
    "normalized": true
  },
  "index": {
    "engine": "usearch",
    "metric": "cosine",
    "document_count": 1234567
  },
  "outputs": [
    {
      "path": "sapbert_cls.usearch",
      "sha256": "..."
    },
    {
      "path": "concept_ids.arrow",
      "sha256": "..."
    }
  ]
}
```

### 7.5 Rebuild triggers

Rebuild when:

- catalog artifact changes
- SapBERT model artifact changes
- pooling changes
- normalization changes
- USearch parameters change
- concept scope changes

## 8. THIRAWAT-SapBERT model artifact

### 8.1 Purpose

THIRAWAT-SapBERT is the Drug-domain mapper encoder. It emits per-token vectors used by Tachiom and exact BiMaxSim.

### 8.2 Files

```text
models/thirawat-sapbert/
  config.json
  tokenizer.json
  tokenizer_config.json
  special_tokens_map.json
  model.safetensors
  colbert_projection.safetensors
  manifest.json
```

### 8.3 Manifest

```json
{
  "artifact_id": "sidataplus-thirawat-sapbert-merged-v1",
  "artifact_kind": "thirawat-sapbert-model",
  "schema_version": "usagi-thirawat-model-v1",
  "api_version": "0.1.0",
  "created_at": "2026-05-31T00:00:00Z",
  "model": {
    "model_id": "sidataplus/THIRAWAT-SapBERT",
    "base_model": "cambridgeltl/SapBERT-UMLS-2020AB-all-lang-from-XLMR",
    "architecture": "pylate_colbert",
    "encoder_family": "xlm-roberta",
    "query_length": 96,
    "document_length": 96,
    "hidden_dim": 768,
    "projection_dim": 128,
    "similarity": "maxsim",
    "peft": {
      "merged": true
    }
  },
  "projection": {
    "in_features": 768,
    "out_features": 128,
    "bias": false
  },
  "outputs": [
    {
      "path": "model.safetensors",
      "sha256": "..."
    },
    {
      "path": "colbert_projection.safetensors",
      "sha256": "..."
    },
    {
      "path": "tokenizer.json",
      "sha256": "..."
    }
  ]
}
```

### 8.4 Runtime rule

The Rust runtime expects merged weights. Runtime PEFT or LoRA adapter merging is out of scope for API v0.1.

## 9. THIRAWAT Drug document embedding artifact

### 9.1 Purpose

This artifact stores document-side token embeddings for standard Drug concepts.

### 9.2 Domain scope

```text
domain_id = Drug
```

Only Drug is supported initially.

Default target scope:

```json
{
  "domain_id": ["Drug"],
  "vocabulary_id": ["RxNorm", "RxNorm Extension"],
  "standard_concept": "S",
  "invalid_reason": null
}
```

### 9.3 Files

```text
mapper/thirawat-drug/doc_embeddings/
  token_vectors.npy
  token_ids.npy
  doclens.npy
  doc_ids.arrow
  manifest.json
```

### 9.4 Manifest

```json
{
  "artifact_id": "athena-20250827-thirawat-drug-docemb-v1",
  "artifact_kind": "thirawat-drug-doc-embeddings",
  "schema_version": "usagi-thirawat-docemb-v1",
  "api_version": "0.1.0",
  "created_at": "2026-05-31T00:00:00Z",
  "catalog": {
    "artifact_id": "athena-20250827-standard-v1",
    "sha256": "..."
  },
  "model": {
    "artifact_id": "sidataplus-thirawat-sapbert-merged-v1",
    "model_id": "sidataplus/THIRAWAT-SapBERT",
    "projection_dim": 128
  },
  "scope": {
    "domain_id": ["Drug"],
    "vocabulary_id": ["RxNorm", "RxNorm Extension"],
    "standard_concept": "S",
    "invalid_reason": null
  },
  "counts": {
    "documents": 123456,
    "tokens": 9876543
  },
  "outputs": [
    {
      "path": "token_vectors.npy",
      "sha256": "..."
    },
    {
      "path": "token_ids.npy",
      "sha256": "..."
    },
    {
      "path": "doclens.npy",
      "sha256": "..."
    },
    {
      "path": "doc_ids.arrow",
      "sha256": "..."
    }
  ]
}
```

### 9.5 Rebuild triggers

Rebuild when:

- catalog artifact changes
- THIRAWAT model artifact changes
- target scope changes
- normalization changes
- token vector format changes

## 10. Tachiom index artifact

### 10.1 Purpose

Tachiom provides fast MaxSim-style retrieval over THIRAWAT token vectors. The mapper then performs external exact BiMaxSim reranking on top candidates.

### 10.2 Files

```text
mapper/thirawat-drug/tachiom/
  index.bin
  manifest.json
```

### 10.3 Manifest

```json
{
  "artifact_id": "athena-20250827-thirawat-drug-tachiom-v1",
  "artifact_kind": "tachiom-index",
  "schema_version": "usagi-tachiom-v1",
  "api_version": "0.1.0",
  "created_at": "2026-05-31T00:00:00Z",
  "input": {
    "doc_embedding_artifact_id": "athena-20250827-thirawat-drug-docemb-v1",
    "sha256": "..."
  },
  "retrieval": {
    "engine": "tachiom",
    "metric": "maxsim"
  },
  "build_params": {
    "token_aware_clustering": true
  },
  "outputs": [
    {
      "path": "index.bin",
      "sha256": "..."
    }
  ]
}
```

### 10.4 Rebuild triggers

Rebuild when:

- THIRAWAT document embedding artifact changes
- Tachiom build parameters change
- Tachiom version changes
- index format version changes

## 11. Job result artifacts

Large job outputs should be written as artifacts.

Example:

```text
jobs/results/job_map_abc/
  results.jsonl
  manifest.json
```

Manifest:

```json
{
  "artifact_id": "job_map_abc_results_v1",
  "artifact_kind": "mapper-batch-results",
  "schema_version": "usagi-mapper-results-v1",
  "api_version": "0.1.0",
  "created_at": "2026-05-31T00:00:00Z",
  "job_id": "job_map_abc",
  "outputs": [
    {
      "path": "results.jsonl",
      "content_type": "application/jsonl",
      "sha256": "..."
    }
  ]
}
```

Each JSONL row:

```json
{
  "id": "src_001",
  "source_code": "SRC001",
  "source_name": "tramadol hydrochloride 50 mg capsule",
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
```

## 12. Artifact validation

Each service validates required artifacts at startup and before use.

### 12.1 Validation steps

1. Manifest exists.
2. Manifest schema version is supported.
3. Referenced files exist.
4. SHA256 checks pass.
5. Catalog artifact IDs match downstream manifests.
6. Model artifact IDs match downstream manifests.
7. Domain scope is compatible.
8. Index reader version is compatible.

### 12.2 Validation failure

Return:

```json
{
  "error": {
    "code": "INCOMPATIBLE_ARTIFACT",
    "message": "Tachiom index was built from a different THIRAWAT document embedding artifact",
    "details": {
      "expected": "athena-20250827-thirawat-drug-docemb-v1",
      "actual": "athena-20250827-thirawat-drug-docemb-v0"
    },
    "request_id": "req_abc"
  }
}
```

## 13. Cleanup policy

Artifacts are immutable by default.

Build jobs write to a temporary path:

```text
data/.tmp/job_<id>/
```

On success:

```text
atomic rename to final artifact path
write manifest
validate manifest
mark job succeeded
```

On failure:

```text
keep temporary path if debug enabled
otherwise delete temporary path
mark job failed
```

Do not mutate a live index in place. That is how search services become haunted.

## 14. Index packs, later

Index packs are not required for API stabilization, but manifests should be compatible with future packs.

Future pack format:

```text
usagi-indexpack-athena-20250827-standard-full.zip
  catalog/
    catalog.sqlite
    manifest.json
  search/
    tantivy/
    sapbert/
  mapper/
    thirawat-drug/
  pack_manifest.json
```

API v0.1 only needs local artifact directories and manifests.

## 15. Security constraints

Build endpoints must not accept arbitrary host paths in production.

Recommended path policy:

```text
allowed_input_roots:
  /fixtures
  /data/imports
  /data/athena

allowed_output_roots:
  /data/catalog
  /data/search
  /data/mapper
  /data/jobs
```

Reject path traversal:

```text
../
absolute paths outside allowed roots
symlink escape
```

Yes, even for local Docker. Local Docker is still software, and software loves becoming a file deletion service when nobody is looking.
