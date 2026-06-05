#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/mapper-smoke"
PORT="${MAPPER_SMOKE_PORT:-8792}"
BASE_URL="http://127.0.0.1:${PORT}"
API_KEY="${MAPPER_SMOKE_API_KEY:-local-mapper-smoke}"
rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/thirawat-drug"

auth_curl() {
  curl -fsS -H "X-API-Key: ${API_KEY}" "$@"
}

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
USAGI_API_KEYS="${API_KEY}" \
cargo run -q -p mapper-api >"${WORK_DIR}/mapper-api.log" 2>&1 &
SERVER_PID=$!
trap 'kill "${SERVER_PID}" 2>/dev/null || true' EXIT

for _ in {1..300}; do
  if curl -fsS "${BASE_URL}/mapper/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
curl -fsS "${BASE_URL}/mapper/health" >/dev/null

auth_curl \
  -H 'Content-Type: application/json' \
  -d '{
    "source_name": "Augmentin 875/125",
    "source_code": "SRC_AUGMENTIN_875_125",
    "mode": "thirawat_tachiom",
    "candidate_top_k": 10,
    "rerank_top_n": 10,
    "limit": 1
  }' \
  "${BASE_URL}/mapper/drugs/query" |
python3 -c 'import json,sys; data=json.load(sys.stdin); concept=data["candidates"][0]["concept"]; assert concept["concept_id"] == 123456; assert "amoxicillin 875 MG / clavulanate 125 MG" in concept["concept_name"]; print(json.dumps({"top_concept_id": concept["concept_id"], "top_concept_name": concept["concept_name"]}))'

auth_curl \
  -H 'Content-Type: application/json' \
  -d '{
    "mode": "thirawat_tachiom",
    "candidate_top_k": 10,
    "rerank_top_n": 10,
    "limit": 1,
    "items": [
      {
        "id": "SRC_AUGMENTIN_875_125",
        "source_name": "Augmentin 875/125",
        "source_code": "SRC_AUGMENTIN_875_125"
      }
    ]
  }' \
  "${BASE_URL}/mapper/drugs/batch" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["items"][0]["candidates"][0]["concept"]["concept_id"] == 123456; assert data["provenance"]["model_artifact_id"] == "sidataplus/THIRAWAT-SapBERT"; assert data["provenance"]["index_artifact_id"].endswith("/manifest.json"); print(json.dumps({"batch_top_concept_id": data["items"][0]["candidates"][0]["concept"]["concept_id"], "batch_provenance": data["provenance"]}))'

auth_curl \
  -H 'Content-Type: application/json' \
  -d '{
    "source_name": "Augmentin 875/125",
    "source_code": "SRC_AUGMENTIN_875_125",
    "mode": "thirawat_tachiom",
    "concept_id": 123456
  }' \
  "${BASE_URL}/mapper/drugs/explain" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["concept"]["concept_id"] == 123456; assert data["token_debug"]["enabled"] is False; assert "bimaxsim" in data["scores"]; assert data["features"]["strength_exact"] is True; print(json.dumps({"explain_concept_id": data["concept"]["concept_id"], "token_debug": data["token_debug"]}))'
