# usagi-api Deployment Manual

Status: draft v0.1  
Audience: API operators, Rails integrators, and developers running the API stack

## Purpose

This manual describes how to run the API-first Rust engine layer as a local or
containerized service set before Rails is introduced.

The deployable API stack is:

| Service | Port | Role |
|---|---:|---|
| `catalog-api` | `8788` | Standard-only OMOP catalog lookup and catalog build jobs |
| `search-api` | `8789` | Tantivy lexical search, SapBERT dense search, and hybrid RRF |
| `mapper-api` | `8790` | Drug mapper query, mapper batch jobs, and mapper job polling |
| `api-worker` | none | Claims queued engine jobs and writes artifacts/results |

Rails talks to these services over HTTP. Rails must not read or mutate
`catalog.sqlite`, indexes, model artifacts, `jobs.sqlite`, or result files
directly.

## Security

All non-probe endpoints require an API key.

Set:

```bash
export USAGI_API_KEYS="replace-with-local-or-deployment-secret"
```

Clients send either:

```http
X-API-Key: replace-with-local-or-deployment-secret
```

or:

```http
Authorization: Bearer replace-with-local-or-deployment-secret
```

`/catalog/health`, `/catalog/status`, `/search/health`, `/search/status`,
`/mapper/health`, and `/mapper/status` stay public for probes. All other
endpoints fail closed when `USAGI_API_KEYS` is unset or empty.

The Docker Compose file publishes ports on `127.0.0.1` by default. Only set
`USAGI_PUBLISH_HOST=0.0.0.0` behind a trusted network boundary or reverse proxy.

## Shared Data Layout

The Compose stack mounts the repo `data/` directory into containers as `/data`.

```text
data/
  catalog/catalog.sqlite
  catalog/manifest.json
  jobs/jobs.sqlite
  jobs/results/
  search/tantivy/index/
  search/sapbert/
  mapper/thirawat-drug/
  models/sapbert/
  models/thirawat-sapbert/
```

Do not commit real runtime data, full vocabulary dumps, model weights, SQLite
databases, or generated indexes.

## Local Docker Compose

Start the API stack:

```bash
USAGI_API_KEYS=smoke-secret \
docker compose -f infra/docker/docker-compose.api.yml up -d --build
```

Run the smoke test:

```bash
USAGI_SMOKE_API_KEY=smoke-secret scripts/smoke-api.sh
```

Stop the stack:

```bash
docker compose -f infra/docker/docker-compose.api.yml down
```

The smoke test checks health, status, catalog build, Tantivy build, job polling,
and a known lexical query against the tiny fixture catalog.

## Running Services Directly

Run each HTTP service with explicit bind addresses and a shared `jobs.sqlite`:

```bash
USAGI_API_KEYS=local-secret \
CATALOG_API_ADDR=127.0.0.1:8788 \
CATALOG_DB_PATH=data/catalog/catalog.sqlite \
JOBS_DB_PATH=data/jobs/jobs.sqlite \
cargo run -p catalog-api
```

```bash
USAGI_API_KEYS=local-secret \
SEARCH_API_ADDR=127.0.0.1:8789 \
CATALOG_DB_PATH=data/catalog/catalog.sqlite \
JOBS_DB_PATH=data/jobs/jobs.sqlite \
TANTIVY_INDEX_DIR=data/search/tantivy/index \
SAPBERT_INDEX_DIR=data/search/sapbert \
cargo run -p search-api
```

```bash
USAGI_API_KEYS=local-secret \
MAPPER_API_ADDR=127.0.0.1:8790 \
JOBS_DB_PATH=data/jobs/jobs.sqlite \
JOB_RESULTS_DIR=data/jobs/results \
THIRAWAT_ARTIFACT_DIR=data/mapper/thirawat-drug \
TACHIOM_INDEX_DIR=data/mapper/thirawat-drug/tachiom \
cargo run -p mapper-api
```

Run the worker:

```bash
JOBS_DB_PATH=data/jobs/jobs.sqlite \
JOB_RESULTS_DIR=data/jobs/results \
cargo run -p api-worker --bin usagi-worker -- --queues catalog,index,embed,map
```

For one-shot local smoke processing:

```bash
cargo run -p api-worker --bin usagi-worker -- --queues map --once
```

## Job Flow

Build and bulk operations are asynchronous:

1. A service creates a job row in `jobs.sqlite`.
2. The service returns `job_id`, `status_url`, and `result_url`.
3. `api-worker` claims the job from the requested queue.
4. The worker writes stages to `/jobs/:id/events`.
5. The worker writes results or immutable artifact references.
6. Rails polls `/jobs/:id` and then reads `/jobs/:id/results`.

For JSONL result artifacts, Rails should request:

```http
Accept: application/jsonl
```

against `/jobs/:id/results`. The API validates the artifact manifest checksum
before streaming the body.

## Required Verification

Before treating a deployment as ready:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
USAGI_API_KEYS=smoke-secret docker compose -f infra/docker/docker-compose.api.yml up -d --build
USAGI_SMOKE_API_KEY=smoke-secret scripts/smoke-api.sh
```

Additional fixture smokes:

```bash
scripts/smoke-search-fixture.sh
scripts/smoke-mapper-fixture.sh
scripts/smoke-mapper-batch-fixture.sh
scripts/smoke-dev-drugs-50.sh
scripts/smoke-dev-drugs-50-batch-job.sh
```

## Rails Integration Defaults

Rails should configure one client per service:

```text
CATALOG_API_URL=http://catalog-api:8788
SEARCH_API_URL=http://search-api:8789
MAPPER_API_URL=http://mapper-api:8790
USAGI_API_KEY=...
ENGINE_API_TIMEOUT_SECONDS=30
ENGINE_API_JOB_POLL_INTERVAL_SECONDS=2
```

Rails should centralize API calls, propagate `X-Request-Id`, attach the API key,
parse the shared error envelope, and mirror engine jobs in Rails-owned tables.

## Troubleshooting

| Symptom | Check |
|---|---|
| `401 UNAUTHORIZED` on protected endpoints | `USAGI_API_KEYS` on the service and `X-API-Key` from the client |
| Build job remains `queued` | `api-worker` is running with the matching queue |
| Search returns `INDEX_NOT_READY` | Tantivy/SapBERT artifact paths and build jobs |
| Mapper returns `MODEL_NOT_READY` | THIRAWAT model artifacts and manifest |
| Mapper returns `INDEX_NOT_READY` | Tachiom manifest and index path |
| Result artifact cannot stream | `/jobs/:id/results` has an artifact and the manifest checksum matches |
