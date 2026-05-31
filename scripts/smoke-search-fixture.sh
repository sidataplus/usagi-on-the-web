#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/search-smoke"
CATALOG_PORT="${SEARCH_SMOKE_CATALOG_PORT:-8786}"
SEARCH_PORT="${SEARCH_SMOKE_SEARCH_PORT:-8787}"
CATALOG_BASE_URL="http://127.0.0.1:${CATALOG_PORT}"
SEARCH_BASE_URL="http://127.0.0.1:${SEARCH_PORT}"
rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/catalog" "${WORK_DIR}/jobs" "${WORK_DIR}/search"

CATALOG_PID=""
SEARCH_PID=""
cleanup() {
  if [[ -n "${CATALOG_PID}" ]]; then kill "${CATALOG_PID}" 2>/dev/null || true; fi
  if [[ -n "${SEARCH_PID}" ]]; then kill "${SEARCH_PID}" 2>/dev/null || true; fi
}
trap cleanup EXIT

CATALOG_DB_PATH="${WORK_DIR}/catalog/catalog.sqlite" \
JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
CATALOG_API_ADDR="127.0.0.1:${CATALOG_PORT}" \
cargo run -q -p catalog-api &
CATALOG_PID=$!

for _ in {1..50}; do
  if curl -fsS "${CATALOG_BASE_URL}/catalog/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
curl -fsS "${CATALOG_BASE_URL}/catalog/health" >/dev/null

curl -fsS \
  -H 'Content-Type: application/json' \
  -d "{\"athena_dir\":\"${ROOT_DIR}/fixtures/search-smoke/athena-mini\",\"idempotency_key\":\"search-smoke-catalog-v1\"}" \
  "${CATALOG_BASE_URL}/catalog/build-job" >/dev/null

CATALOG_DB_PATH="${WORK_DIR}/catalog/catalog.sqlite" \
JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
cargo run -q -p api-worker --bin usagi-worker -- --queues catalog --once >/dev/null

CATALOG_DB_PATH="${WORK_DIR}/catalog/catalog.sqlite" \
JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
TANTIVY_INDEX_DIR="${WORK_DIR}/search/tantivy/index" \
SAPBERT_INDEX_DIR="${WORK_DIR}/search/sapbert" \
SAPBERT_QUERY_EMBEDDINGS_PATH="${ROOT_DIR}/fixtures/search-smoke/embeddings/sapbert-query-embeddings.json" \
SEARCH_API_ADDR="127.0.0.1:${SEARCH_PORT}" \
cargo run -q -p search-api &
SEARCH_PID=$!

for _ in {1..50}; do
  if curl -fsS "${SEARCH_BASE_URL}/search/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
curl -fsS "${SEARCH_BASE_URL}/search/health" >/dev/null

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"idempotency_key":"search-smoke-sapbert-v1"}' \
  "${SEARCH_BASE_URL}/search/sapbert/build-job" >/dev/null

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"idempotency_key":"search-smoke-tantivy-v1"}' \
  "${SEARCH_BASE_URL}/search/tantivy/build-job" >/dev/null

CATALOG_DB_PATH="${WORK_DIR}/catalog/catalog.sqlite" \
JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
TANTIVY_INDEX_DIR="${WORK_DIR}/search/tantivy/index" \
cargo run -q -p api-worker --bin usagi-worker -- --queues index --once >/dev/null

CATALOG_DB_PATH="${WORK_DIR}/catalog/catalog.sqlite" \
JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
SAPBERT_INDEX_DIR="${WORK_DIR}/search/sapbert" \
SAPBERT_PRECOMPUTED_EMBEDDINGS_PATH="${ROOT_DIR}/fixtures/search-smoke/embeddings/sapbert-doc-embeddings.json" \
cargo run -q -p api-worker --bin usagi-worker -- --queues embed --once >/dev/null

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"q":"tramadol 50 mg capsule","mode":"sapbert_cls","limit":1}' \
  "${SEARCH_BASE_URL}/search/concepts" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["results"][0]["concept"]["concept_id"] == 100; print(json.dumps({"top_concept_id": data["results"][0]["concept"]["concept_id"]}))'

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"mode":"lexical_tantivy","limit_per_item":1,"items":[{"id":"q1","q":"tramadol 50 mg capsule"}]}' \
  "${SEARCH_BASE_URL}/search/batch" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["provenance"]["catalog_artifact_id"] == "local-catalog-standard-v1"; assert data["provenance"]["index_artifact_id"] == "local-tantivy-v1"; print(json.dumps({"batch_provenance": data["provenance"]}))'

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"mode":"sapbert_cls","limit_per_item":1,"items":[{"id":"q1","q":"tramadol 50 mg capsule"}]}' \
  "${SEARCH_BASE_URL}/search/batch" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["items"][0]["results"][0]["concept"]["concept_id"] == 100; assert data["provenance"]["index_artifact_id"] == "local-sapbert-cls-v1"; print(json.dumps({"sapbert_batch_top_concept_id": data["items"][0]["results"][0]["concept"]["concept_id"]}))'

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"mode":"hybrid_rrf","limit_per_item":1,"items":[{"id":"q1","q":"tramadol 50 mg capsule"}]}' \
  "${SEARCH_BASE_URL}/search/batch" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["items"][0]["results"][0]["concept"]["concept_id"] == 100; assert data["provenance"]["index_artifact_id"] == "local-hybrid-rrf-v1"; print(json.dumps({"hybrid_batch_top_concept_id": data["items"][0]["results"][0]["concept"]["concept_id"]}))'

curl -fsS \
  -H 'Content-Type: application/json' \
  -d '{"q":"tramadol 50 mg capsule","mode":"hybrid_rrf","concept_id":100}' \
  "${SEARCH_BASE_URL}/search/explain" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["concept_id"] == 100; assert "hybrid_rrf" in data["explanation"]; assert "sapbert" in data["explanation"]; print(json.dumps({"hybrid_explain_concept_id": data["concept_id"]}))'
