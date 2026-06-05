# API Artifact Build Manual

Status: draft v0.1  
Audience: API operators and deployment maintainers

## Purpose

This manual describes how to create the runtime artifacts needed by
`usagi-api`:

```text
Athena files
  -> catalog.sqlite
  -> Tantivy lexical index
  -> SapBERT CLS embeddings + USearch index
  -> THIRAWAT Drug document embeddings
  -> Tachiom Drug index
```

Build these on the API compute server. Rails must not read, write, or rebuild
these artifacts directly.

## Preconditions

Prepare the API data volume:

```bash
mkdir -p \
  data/athena \
  data/catalog \
  data/jobs/results \
  data/search/tantivy/index \
  data/search/sapbert \
  data/mapper/thirawat-drug \
  data/models/sapbert \
  data/models/thirawat-sapbert
```

Place Athena vocabulary files under `data/athena/<version>/`:

```text
CONCEPT.csv
CONCEPT_SYNONYM.csv
CONCEPT_RELATIONSHIP.csv
CONCEPT_ANCESTOR.csv
VOCABULARY.csv
DOMAIN.csv
CONCEPT_CLASS.csv
RELATIONSHIP.csv
```

Place model artifacts under:

```text
data/models/sapbert/
data/models/thirawat-sapbert/
```

Do not commit these files.

The examples below use local API-key auth bound to `127.0.0.1` for a compute
server maintenance session. For production serving, switch the API services to
`USAGI_API_AUTH_MODE=signed` and use the same `USAGI_API_SHARED_SECRET` as
Rails.

## Start Services For Builds

Start the API stack:

```bash
USAGI_API_AUTH_MODE=api_key \
USAGI_API_KEYS=local-build-secret \
docker compose -f infra/docker/docker-compose.api.yml up -d --build
```

The Compose stack starts `api-worker` with the build queues configured in
`infra/docker/docker-compose.api.yml`. If you run services directly instead of
Compose, start a worker that can claim every build queue:

```bash
JOBS_DB_PATH=data/jobs/jobs.sqlite \
JOB_RESULTS_DIR=data/jobs/results \
CATALOG_DB_PATH=data/catalog/catalog.sqlite \
TANTIVY_INDEX_DIR=data/search/tantivy/index \
SAPBERT_INDEX_DIR=data/search/sapbert \
SAPBERT_MODEL_DIR=data/models/sapbert \
THIRAWAT_MODEL_DIR=data/models/thirawat-sapbert \
THIRAWAT_ARTIFACT_DIR=data/mapper/thirawat-drug \
TACHIOM_INDEX_DIR=data/mapper/thirawat-drug/tachiom \
cargo run -p api-worker --bin usagi-worker -- --queues catalog,index,embed,map
```

For one-at-a-time direct maintenance builds, add `--once` and rerun the worker
after each submitted job.

## Poll A Build Job

Each build endpoint returns a `job_id`. Poll it from the service that created
the job:

```bash
curl -fsS -H 'X-API-Key: local-build-secret' \
  http://127.0.0.1:8788/jobs/job_catalog_abc
```

Wait for:

```text
state = succeeded
```

Stop and inspect logs if a job ends as `failed`, `cancelled`, or
`succeeded_with_errors`.

## 1. Build Catalog SQLite

Create the catalog build job:

```bash
curl -fsS \
  -H 'X-API-Key: local-build-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "athena_dir": "/data/athena/20250827",
    "idempotency_key": "catalog-athena-20250827-standard-v1",
    "overwrite": false
  }' \
  http://127.0.0.1:8788/catalog/build-job
```

Expected outputs:

```text
data/catalog/catalog.sqlite
data/catalog/manifest.json
```

Verify:

```bash
curl -fsS http://127.0.0.1:8788/catalog/status
```

The catalog build keeps only valid standard concepts:

```sql
standard_concept = 'S'
AND invalid_reason IS NULL
```

## 2. Build Tantivy Lexical Index

Create the Tantivy build job after the catalog is ready:

```bash
curl -fsS \
  -H 'X-API-Key: local-build-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "idempotency_key": "tantivy-athena-20250827-standard-v1",
    "overwrite": false,
    "schema_version": "usagi-tantivy-v1"
  }' \
  http://127.0.0.1:8789/search/tantivy/build-job
```

Expected outputs:

```text
data/search/tantivy/index/
data/search/tantivy/manifest.json
```

Verify:

```bash
curl -fsS http://127.0.0.1:8789/search/status
```

## 3. Build SapBERT CLS + USearch

Create the SapBERT build job after the catalog and SapBERT model artifact are
ready:

```bash
curl -fsS \
  -H 'X-API-Key: local-build-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "idempotency_key": "sapbert-athena-20250827-standard-v1",
    "overwrite": false,
    "model_artifact_id": "sapbert-xlmr-merged-v1",
    "scope": {
      "standard_concept": "S",
      "invalid_reason": null
    },
    "batch_size": 32
  }' \
  http://127.0.0.1:8789/search/sapbert/build-job
```

Expected outputs:

```text
data/search/sapbert/sapbert_cls.usearch
data/search/sapbert/concept_ids.arrow
data/search/sapbert/manifest.json
```

Verify semantic search:

```bash
curl -fsS \
  -H 'X-API-Key: local-build-secret' \
  -H 'Content-Type: application/json' \
  -d '{"q":"tramadol 50 mg capsule","mode":"sapbert_cls","limit":3}' \
  http://127.0.0.1:8789/search/concepts
```

## 4. Build THIRAWAT Drug Document Embeddings

This is the GPU-targeted stage. It can run on the API compute server, but for a
full Athena Drug/RxNorm build the preferred operator path is to run it remotely
on Modal and then import the generated `doc_embeddings` artifact back into the
runtime `/data` volume.

The current Rust encoder path still uses Candle on CPU; the Modal pipeline is
wired so the stage can move to GPU execution without changing artifact layout
when CUDA support is enabled in `usagi-embed`.

Create the THIRAWAT document embedding job after the catalog and THIRAWAT model
artifact are ready:

```bash
curl -fsS \
  -H 'X-API-Key: local-build-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "idempotency_key": "thirawat-docemb-athena-20250827-drug-v1",
    "overwrite": false,
    "domain_id": "Drug",
    "model_artifact_id": "sidataplus-thirawat-sapbert-merged-v1",
    "batch_size": 16,
    "target_scope": {
      "domain_id": ["Drug"],
      "vocabulary_id": ["RxNorm", "RxNorm Extension"]
    }
  }' \
  http://127.0.0.1:8790/mapper/thirawat/build-embeddings-job
```

Expected outputs:

```text
data/mapper/thirawat-drug/doc_embeddings/token_vectors.npy
data/mapper/thirawat-drug/doc_embeddings/token_ids.npy
data/mapper/thirawat-drug/doc_embeddings/doclens.npy
data/mapper/thirawat-drug/doc_embeddings/doc_ids.arrow
data/mapper/thirawat-drug/doc_embeddings/manifest.json
```

### Remote GPU Build With Modal

Use Modal when the serving API host does not have enough GPU/CPU/RAM for the
document embedding build, or when the build should run as a disposable
maintenance job.

The Modal path reuses the same API job flow:

```text
Modal Volume /data
  -> mapper-api
  -> /mapper/thirawat/build-embeddings-job
  -> api-worker --queues embed --once
  -> data/mapper/thirawat-drug/doc_embeddings/
  -> /exports/usagi-thirawat-docemb-*.tar.zst
```

Stage only the required inputs into the Modal Volume:

```text
data/catalog/catalog.sqlite
data/catalog/manifest.json
data/models/thirawat-sapbert/
```

Run:

```bash
modal volume create usagi-artifacts-data
modal volume put usagi-artifacts-data ./data/catalog /data/catalog
modal volume put usagi-artifacts-data ./data/models/thirawat-sapbert /data/models/thirawat-sapbert

USAGI_MODAL_GPU=L40S \
uv run infra/modal/modal_artifact_pipeline.py \
  --idempotency-key thirawat-docemb-athena-20250827-drug-v1 \
  --artifact-id athena-20250827-thirawat-drug-docemb-v1 \
  --catalog-artifact-id athena-20250827-standard-v1 \
  --model-artifact-id sidataplus-thirawat-sapbert-merged-v1 \
  --batch-size 16
```

Expected Modal outputs:

```text
/data/mapper/thirawat-drug/doc_embeddings/
/exports/usagi-thirawat-docemb-athena-20250827-thirawat-drug-docemb-v1.tar.zst
/exports/usagi-thirawat-docemb-athena-20250827-thirawat-drug-docemb-v1.tar.zst.manifest.json
```

Download and restore on the API compute server:

```bash
modal volume get usagi-artifacts-data \
  /exports/usagi-thirawat-docemb-athena-20250827-thirawat-drug-docemb-v1.tar.zst \
  ./artifacts/

tar --zstd -xf artifacts/usagi-thirawat-docemb-athena-20250827-thirawat-drug-docemb-v1.tar.zst \
  -C data
```

Then continue with Stage 5 on the API compute server. See
`infra/modal/README.md` for Hugging Face upload, Volume layout, and restore
details.

Keep Modal exports private unless Athena vocabulary, model, and derived
embedding redistribution rights have been reviewed.

## 5. Build Tachiom Index

Create the Tachiom build job after THIRAWAT document embeddings are ready. If
Stage 4 ran on Modal, first restore the downloaded pack so the runtime host has:

```text
data/mapper/thirawat-drug/doc_embeddings/manifest.json
data/mapper/thirawat-drug/doc_embeddings/token_vectors.npy
data/mapper/thirawat-drug/doc_embeddings/token_ids.npy
data/mapper/thirawat-drug/doc_embeddings/doclens.npy
data/mapper/thirawat-drug/doc_embeddings/doc_ids.arrow
```

```bash
curl -fsS \
  -H 'X-API-Key: local-build-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "idempotency_key": "tachiom-athena-20250827-thirawat-drug-v1",
    "overwrite": false,
    "doc_embedding_artifact_id": "athena-20250827-thirawat-drug-docemb-v1",
    "build_params": {
      "metric": "maxsim",
      "token_aware_clustering": true
    }
  }' \
  http://127.0.0.1:8790/mapper/tachiom/build-index-job
```

Expected outputs:

```text
data/mapper/thirawat-drug/tachiom/index.bin
data/mapper/thirawat-drug/tachiom/manifest.json
```

Verify mapper readiness:

```bash
curl -fsS http://127.0.0.1:8790/mapper/status
```

## Final Smoke

Run fixture smokes only against disposable local data, not a production artifact
volume:

```bash
scripts/smoke-search-fixture.sh
scripts/smoke-mapper-fixture.sh
scripts/smoke-dev-drugs-50.sh
```

For the full local deployment topology, use a non-production data volume and
then run:

```bash
scripts/smoke-deploy.sh
```

## Switching To Production Serving

After artifacts are ready, restart API services with production signed auth:

```bash
USAGI_API_ENV=production \
USAGI_API_AUTH_MODE=signed \
USAGI_API_SHARED_SECRET=replace-with-shared-hmac-secret \
docker compose -f infra/docker/docker-compose.api.yml up -d
```

Do not expose engine ports publicly. For DigitalOcean Rails plus local compute,
publish engine ports only on the Tailscale interface or keep them behind a
Tailscale-only firewall rule.
