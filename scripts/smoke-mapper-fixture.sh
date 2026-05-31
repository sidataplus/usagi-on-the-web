#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/mapper-smoke"
PORT="${MAPPER_SMOKE_PORT:-8792}"
BASE_URL="http://127.0.0.1:${PORT}"
rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/thirawat-drug"

cp -R "${ROOT_DIR}/fixtures/mapper-smoke/doc_embeddings" "${WORK_DIR}/thirawat-drug/doc_embeddings"
cp -R "${ROOT_DIR}/fixtures/mapper-smoke/query_embeddings" "${WORK_DIR}/thirawat-drug/query_embeddings"

THIRAWAT_ARTIFACT_DIR="${WORK_DIR}/thirawat-drug" \
THIRAWAT_DOC_EMBEDDING_DIR="${WORK_DIR}/thirawat-drug/doc_embeddings" \
TACHIOM_INDEX_DIR="${WORK_DIR}/thirawat-drug/tachiom" \
TACHIOM_ARTIFACT_ID="fixture-thirawat-drug-tachiom-v1" \
TACHIOM_BACKEND=fixture \
cargo run -q -p usagi-tachiom --bin usagi-tachiom-build

THIRAWAT_ARTIFACT_DIR="${WORK_DIR}/thirawat-drug" \
TACHIOM_INDEX_DIR="${WORK_DIR}/thirawat-drug/tachiom" \
THIRAWAT_QUERY_EMBEDDINGS_PATH="${WORK_DIR}/thirawat-drug/query_embeddings/query_embeddings.json" \
MAPPER_API_ADDR="127.0.0.1:${PORT}" \
cargo run -q -p mapper-api >"${WORK_DIR}/mapper-api.log" 2>&1 &
SERVER_PID=$!
trap 'kill "${SERVER_PID}" 2>/dev/null || true' EXIT

for _ in {1..100}; do
  if curl -fsS "${BASE_URL}/mapper/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
curl -fsS "${BASE_URL}/mapper/health" >/dev/null

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{
    "source_name": "tramadol hydrochloride 50 mg capsule",
    "source_code": "SRC_TRAMADOL_50_CAP",
    "mode": "thirawat_tachiom",
    "candidate_top_k": 10,
    "rerank_top_n": 10,
    "limit": 1
  }' \
  "${BASE_URL}/mapper/drugs/query" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["candidates"][0]["concept"]["concept_id"] == 40162522; print(json.dumps({"top_concept_id": data["candidates"][0]["concept"]["concept_id"]}))'

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{
    "mode": "thirawat_tachiom",
    "candidate_top_k": 10,
    "rerank_top_n": 10,
    "limit": 1,
    "items": [
      {
        "id": "SRC_TRAMADOL_50_CAP",
        "source_name": "tramadol hydrochloride 50 mg capsule",
        "source_code": "SRC_TRAMADOL_50_CAP"
      }
    ]
  }' \
  "${BASE_URL}/mapper/drugs/batch" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["items"][0]["candidates"][0]["concept"]["concept_id"] == 40162522; assert data["provenance"]["model_artifact_id"] == "sidataplus/THIRAWAT-SapBERT"; assert data["provenance"]["index_artifact_id"].endswith("/manifest.json"); print(json.dumps({"batch_top_concept_id": data["items"][0]["candidates"][0]["concept"]["concept_id"], "batch_provenance": data["provenance"]}))'

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{
    "source_name": "tramadol hydrochloride 50 mg capsule",
    "source_code": "SRC_TRAMADOL_50_CAP",
    "mode": "thirawat_tachiom",
    "concept_id": 40162522
  }' \
  "${BASE_URL}/mapper/drugs/explain" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["concept"]["concept_id"] == 40162522; assert data["token_debug"]["enabled"] is False; assert "bimaxsim" in data["scores"]; print(json.dumps({"explain_concept_id": data["concept"]["concept_id"], "token_debug": data["token_debug"]}))'
