# Modal Artifact Build Notes

These notes wire the GPU-targeted SapBERT and THIRAWAT/Tachiom artifact stages
to Modal.
The source of truth for artifact order is
`docs/manual/api/05-artifact-builds.md`.

The Modal app lives in:

```text
infra/modal/modal_artifact_pipeline.py
```

It runs the existing Rust build paths:

```text
search-api
  -> POST /search/sapbert/build-job
api-worker --queues embed --once
  -> data/search/sapbert/
  -> exported .tar.zst pack

mapper-api
  -> POST /mapper/thirawat/build-embeddings-job
api-worker --queues embed --once
  -> data/mapper/thirawat-drug/doc_embeddings/
mapper-api
  -> POST /mapper/tachiom/build-index-job
api-worker --queues index --once
  -> data/mapper/thirawat-drug/tachiom/
  -> exported .tar.zst pack
```

Rails never reads or writes these artifacts.

## Current Runtime Caveat

`crates/usagi-embed` currently uses `candle_core::Device::Cpu` for XLM-R
encoding. The Modal pipeline is ready to run this stage remotely and requests a
GPU, but the Rust encoder needs a future CUDA feature switch before it consumes
GPU compute. Keep the output contract unchanged when that switch is added.

## Install

Use `uv` or a Python virtualenv:

```bash
uv run --with modal --with requests --with huggingface_hub modal --help
```

Authenticate Modal:

```bash
modal token new
```

Optional Hugging Face upload requires `HF_TOKEN` as a Modal secret attached to
the function:

```bash
modal secret create hf-token HF_TOKEN=...
export USAGI_MODAL_HF_SECRET=hf-token
```

## Stage Inputs

Create or reuse the Modal Volume:

```bash
export USAGI_MODAL_VOLUME=usagi-artifacts-data
modal volume create "${USAGI_MODAL_VOLUME}"
```

Build `catalog.sqlite` locally first. Do not upload the full Athena vocabulary
CSV directory to Modal unless a future build stage genuinely needs raw CSVs.

For the 20260227 vocabulary used in the current build:

```bash
export VOCAB_VERSION=20260227
export VOCAB_SOURCE=/Users/na399/Downloads/vocabulary_v20260227
export VOCAB_STAGE=temp/vocab-${VOCAB_VERSION}

mkdir -p "${VOCAB_STAGE}/data/catalog" "${VOCAB_STAGE}/data/jobs/results"

USAGI_API_ENV=development \
USAGI_API_AUTH_MODE=api_key \
USAGI_API_KEYS=local-build-secret \
CATALOG_API_ADDR=127.0.0.1:8878 \
CATALOG_DB_PATH="${VOCAB_STAGE}/data/catalog/catalog.sqlite" \
JOBS_DB_PATH="${VOCAB_STAGE}/data/jobs/jobs.sqlite" \
JOB_RESULTS_DIR="${VOCAB_STAGE}/data/jobs/results" \
./target/debug/catalog-api
```

In another terminal:

```bash
curl -fsS \
  -H 'X-API-Key: local-build-secret' \
  -H 'Content-Type: application/json' \
  -d "{
    \"athena_dir\": \"${VOCAB_SOURCE}\",
    \"idempotency_key\": \"catalog-athena-${VOCAB_VERSION}-standard-v1\",
    \"overwrite\": true,
    \"vocabulary_version\": \"${VOCAB_VERSION}\",
    \"artifact_id\": \"athena-${VOCAB_VERSION}-standard-v1\"
  }" \
  http://127.0.0.1:8878/catalog/build-job

USAGI_API_ENV=development \
CATALOG_DB_PATH="${VOCAB_STAGE}/data/catalog/catalog.sqlite" \
JOBS_DB_PATH="${VOCAB_STAGE}/data/jobs/jobs.sqlite" \
JOB_RESULTS_DIR="${VOCAB_STAGE}/data/jobs/results" \
./target/debug/usagi-worker --queues catalog --once
```

Verify before upload:

```bash
curl -fsS -H 'X-API-Key: local-build-secret' \
  http://127.0.0.1:8878/catalog/status

ls -lh "${VOCAB_STAGE}/data/catalog/catalog.sqlite" \
  "${VOCAB_STAGE}/data/catalog/manifest.json"
```

Stage the minimum inputs under `/data` in the Volume:

```text
data/catalog/catalog.sqlite
data/catalog/manifest.json
data/models/sapbert/config.json
data/models/sapbert/tokenizer.json
data/models/sapbert/model.safetensors
data/models/thirawat-sapbert/config.json
data/models/thirawat-sapbert/tokenizer.json
data/models/thirawat-sapbert/tokenizer_config.json
data/models/thirawat-sapbert/special_tokens_map.json
data/models/thirawat-sapbert/model.safetensors
data/models/thirawat-sapbert/colbert_projection.safetensors
data/bin/<tachiom build binary>
```

Upload from a local artifact staging directory:

```bash
modal volume put "${USAGI_MODAL_VOLUME}" "${VOCAB_STAGE}/data/catalog" /data/catalog
modal volume put "${USAGI_MODAL_VOLUME}" ./data/models/sapbert /data/models/sapbert
modal volume put "${USAGI_MODAL_VOLUME}" ./data/models/thirawat-sapbert /data/models/thirawat-sapbert
modal volume put "${USAGI_MODAL_VOLUME}" ./data/bin/tachiom-build /data/bin/tachiom-build
```

Set `TACHIOM_BUILD_BIN=/mnt/usagi/data/bin/tachiom-build` before running a
production THIRAWAT/Tachiom build. The Modal script intentionally refuses to use
the fixture Tachiom backend for full vocabulary builds.

## Run The Build

Run SapBERT only:

```bash
USAGI_MODAL_GPU=L40S \
uv run --with modal --with requests --with huggingface_hub \
  modal run infra/modal/modal_artifact_pipeline.py \
  --mode sapbert \
  --vocabulary-version "${VOCAB_VERSION}"
```

Run THIRAWAT document embeddings plus Tachiom:

```bash
USAGI_MODAL_GPU=L40S \
TACHIOM_BUILD_BIN=/mnt/usagi/data/bin/tachiom-build \
uv run --with modal --with requests --with huggingface_hub \
  modal run infra/modal/modal_artifact_pipeline.py \
  --mode thirawat \
  --vocabulary-version "${VOCAB_VERSION}" \
  --thirawat-batch-size 16
```

Run all remote indexes:

```bash
USAGI_MODAL_GPU=L40S \
TACHIOM_BUILD_BIN=/mnt/usagi/data/bin/tachiom-build \
uv run --with modal --with requests --with huggingface_hub \
  modal run infra/modal/modal_artifact_pipeline.py \
  --mode all \
  --vocabulary-version "${VOCAB_VERSION}" \
  --sapbert-batch-size 32 \
  --thirawat-batch-size 16
```

Expected output in the Modal Volume:

```text
/data/search/sapbert/
/data/mapper/thirawat-drug/doc_embeddings/
/data/mapper/thirawat-drug/tachiom/
/exports/usagi-*.tar.zst
/exports/usagi-*.tar.zst.manifest.json
```

## Upload To A Private Hugging Face Dataset

Use a private Dataset repo by default:

```bash
modal secret create hf-token HF_TOKEN=...

USAGI_MODAL_GPU=L40S \
USAGI_MODAL_HF_SECRET=hf-token \
uv run --with modal --with requests --with huggingface_hub \
  modal run infra/modal/modal_artifact_pipeline.py \
  --mode all \
  --vocabulary-version "${VOCAB_VERSION}" \
  --hf-repo-id sidataplus/usagi-athena-20250827-artifacts \
  --hf-private
```

For very large complete `/data` packs, prefer the Hugging Face CLI from an
operator machine:

```bash
HF_XET_HIGH_PERFORMANCE=1 \
hf upload-large-folder sidataplus/usagi-athena-20250827-artifacts ./artifact-pack \
  --repo-type dataset \
  --num-workers 16
```

The Dataset must include:

```text
README.md
pack_manifest.json
per-artifact manifest.json files
checksums
license/access notes
```

## Download And Restore

Download from the Modal Volume:

```bash
modal volume get "${USAGI_MODAL_VOLUME}" \
  /exports/usagi-thirawat-docemb-athena-20250827-thirawat-drug-docemb-v1.tar.zst \
  ./artifacts/
```

Restore into the runtime API host:

```bash
mkdir -p data/mapper/thirawat-drug
tar --zstd -xf artifacts/usagi-thirawat-docemb-athena-20250827-thirawat-drug-docemb-v1.tar.zst \
  -C data
```

Then build the Tachiom index locally on the API compute host:

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

## Distribution Policy

Keep artifact packs private unless the operator has confirmed that:

```text
Athena vocabulary redistribution is permitted
model artifact redistribution is permitted
derived embedding redistribution is permitted
the target registry/dataset access controls are correct
```

Public Docker images or public Hugging Face Datasets should require an explicit
release decision.
