# Modal Artifact Build Notes

These notes wire the GPU-targeted THIRAWAT document embedding stage to Modal.
The source of truth for artifact order is
`docs/manual/api/05-artifact-builds.md`.

The Modal app lives in:

```text
infra/modal/modal_artifact_pipeline.py
```

It runs the existing Rust build path:

```text
mapper-api
  -> POST /mapper/thirawat/build-embeddings-job
api-worker --queues embed --once
  -> data/mapper/thirawat-drug/doc_embeddings/
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
uv run infra/modal/modal_artifact_pipeline.py --help
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

Stage the minimum inputs under `/data` in the Volume:

```text
data/catalog/catalog.sqlite
data/catalog/manifest.json
data/models/thirawat-sapbert/config.json
data/models/thirawat-sapbert/tokenizer.json
data/models/thirawat-sapbert/tokenizer_config.json
data/models/thirawat-sapbert/special_tokens_map.json
data/models/thirawat-sapbert/model.safetensors
data/models/thirawat-sapbert/colbert_projection.safetensors
```

Upload from a local artifact staging directory:

```bash
modal volume put "${USAGI_MODAL_VOLUME}" ./data/catalog /data/catalog
modal volume put "${USAGI_MODAL_VOLUME}" ./data/models/thirawat-sapbert /data/models/thirawat-sapbert
```

Do not upload full Athena CSVs unless the build needs them. The Modal stage only
needs the already-built catalog SQLite and THIRAWAT model artifact.

## Run The Build

Run with the default GPU class:

```bash
uv run infra/modal/modal_artifact_pipeline.py
```

Override IDs and batch size:

```bash
USAGI_MODAL_GPU=L40S \
uv run infra/modal/modal_artifact_pipeline.py \
  --idempotency-key thirawat-docemb-athena-20250827-drug-v1 \
  --artifact-id athena-20250827-thirawat-drug-docemb-v1 \
  --catalog-artifact-id athena-20250827-standard-v1 \
  --model-artifact-id sidataplus-thirawat-sapbert-merged-v1 \
  --batch-size 16
```

Expected output in the Modal Volume:

```text
/data/mapper/thirawat-drug/doc_embeddings/
/exports/usagi-thirawat-docemb-<artifact_id>.tar.zst
/exports/usagi-thirawat-docemb-<artifact_id>.tar.zst.manifest.json
```

## Upload To A Private Hugging Face Dataset

Use a private Dataset repo by default:

```bash
modal secret create hf-token HF_TOKEN=...

USAGI_MODAL_GPU=L40S \
USAGI_MODAL_HF_SECRET=hf-token \
uv run infra/modal/modal_artifact_pipeline.py \
  --artifact-id athena-20250827-thirawat-drug-docemb-v1 \
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
