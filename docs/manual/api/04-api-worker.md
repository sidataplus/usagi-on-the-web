# api-worker Manual

Status: draft v0.1  
Audience: API operators and developers

## Purpose

`api-worker` executes asynchronous engine jobs created by the HTTP services.

It has no HTTP port. It claims queued jobs from `jobs.sqlite`, runs the relevant
stage, writes events and artifacts, then marks jobs complete or failed.

## Ownership

`api-worker` owns execution for:

| Job kind | Queue | Output |
|---|---|---|
| `catalog_build` | `catalog` | `catalog.sqlite`, catalog manifest |
| `tantivy_build` | `index` | Tantivy index and manifest |
| `sapbert_build` | `embed` | SapBERT dense artifact |
| `thirawat_doc_embed` | `embed` | THIRAWAT Drug document token artifacts |
| `tachiom_build` | `index` | Tachiom index and manifest |
| `mapper_drugs_batch` | `map` | JSONL result artifact |

The worker does not expose API endpoints. Job polling stays on the service that
created the job.

## Runtime Configuration

| Variable | Default | Meaning |
|---|---|---|
| `JOBS_DB_PATH` | `data/jobs/jobs.sqlite` | Shared API job store |
| `JOB_RESULTS_DIR` | `data/jobs/results` | Result artifact root |
| `WORKER_QUEUES` | `catalog,index,embed,map` in Compose | Default queue list |
| `CATALOG_DB_PATH` | `data/catalog/catalog.sqlite` | Catalog path |
| `TANTIVY_INDEX_DIR` | `data/search/tantivy/index` | Tantivy output |
| `SAPBERT_INDEX_DIR` | `data/search/sapbert` | SapBERT output |
| `SAPBERT_MODEL_DIR` | `data/models/sapbert` | SapBERT model artifact |
| `SAPBERT_PRECOMPUTED_EMBEDDINGS_PATH` | unset | Fixture doc embeddings |
| `THIRAWAT_MODEL_DIR` | `data/models/thirawat-sapbert` | THIRAWAT model artifact |
| `THIRAWAT_ARTIFACT_DIR` | `data/mapper/thirawat-drug` | Mapper artifact root |
| `TACHIOM_INDEX_DIR` | `data/mapper/thirawat-drug/tachiom` | Tachiom output |
| `TACHIOM_BACKEND` | fixture/default backend | Tachiom build backend |
| `TACHIOM_BUILD_BIN` | unset | Native Tachiom build binary for CLI backend |
| `THIRAWAT_QUERY_EMBEDDINGS_PATH` | unset | Mapper fixture query embeddings |

## Running the Worker

Run all queues:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues catalog,index,embed,map
```

Run selected queues:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues map
```

Run once for smoke tests:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues catalog --once
```

Docker Compose starts:

```text
usagi-worker --queues catalog,index,embed,map
```

## Queue Runbooks

### Catalog

Inputs:

```text
CATALOG_DB_PATH
athena_dir from /catalog/build-job payload
```

Command:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues catalog --once
```

Stages:

```text
validating_input
writing_sqlite
validating_artifact
```

### Tantivy

Inputs:

```text
CATALOG_DB_PATH
TANTIVY_INDEX_DIR
```

Command:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues index --once
```

Stages:

```text
validating_catalog
building_index
validating_artifact
```

### SapBERT

Inputs:

```text
CATALOG_DB_PATH
SAPBERT_INDEX_DIR
SAPBERT_MODEL_DIR
SAPBERT_PRECOMPUTED_EMBEDDINGS_PATH for fixture runs
```

Command:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues embed --once
```

Stages:

```text
validating_model
validating_catalog
embedding_documents
building_usearch_index
validating_artifact
```

### THIRAWAT and Tachiom

Inputs:

```text
CATALOG_DB_PATH
THIRAWAT_MODEL_DIR
THIRAWAT_ARTIFACT_DIR
TACHIOM_INDEX_DIR
```

Commands:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues embed --once
cargo run -p api-worker --bin usagi-worker -- --queues index --once
```

Use `TACHIOM_BACKEND=cli`, `TACHIOM_BUILD_BIN`, and `TACHIOM_SEARCH_BIN` only
when native upstream Tachiom binaries are available.

### Mapper Batch

Inputs:

```text
JOBS_DB_PATH
JOB_RESULTS_DIR
THIRAWAT_ARTIFACT_DIR
TACHIOM_INDEX_DIR
THIRAWAT_QUERY_EMBEDDINGS_PATH for fixture runs
```

Command:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues map --once
```

Stages:

```text
validating_indexes
embedding_queries
tachiom_retrieval
bimaxsim_reranking
deterministic_tiebreak
writing_results
validating_results
```

## Operational Notes

- Run at least one worker process with each queue used by exposed admin actions.
- Build/index jobs are idempotent by API key and payload idempotency key.
- Partial mapper item failures produce `succeeded_with_errors`, not a total job
  failure, unless the whole stage is unusable.
- Workers write JSONL result artifacts under `JOB_RESULTS_DIR`.
- Rails should fetch result artifacts through `/jobs/:id/results` with
  `Accept: application/jsonl`, not through the filesystem.

## Verification

Run:

```bash
cargo test -p api-worker -p usagi-jobs
scripts/smoke-search-fixture.sh
scripts/smoke-mapper-batch-fixture.sh
scripts/smoke-dev-drugs-50-batch-job.sh
```

The mapper batch smokes exercise worker claiming, job events, item-level success
and failure state, result artifact writing, checksum validation, and HTTP JSONL
streaming.
