#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/mapper-batch-smoke"
PORT="${MAPPER_BATCH_SMOKE_PORT:-8791}"
BASE_URL="http://127.0.0.1:${PORT}"
API_KEY="${MAPPER_BATCH_SMOKE_API_KEY:-local-mapper-batch-smoke}"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/thirawat-drug" "${WORK_DIR}/jobs" "${WORK_DIR}/results"

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

MAPPER_API_ADDR="127.0.0.1:${PORT}" \
JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
JOB_RESULTS_DIR="${WORK_DIR}/results" \
THIRAWAT_ARTIFACT_DIR="${WORK_DIR}/thirawat-drug" \
TACHIOM_INDEX_DIR="${WORK_DIR}/thirawat-drug/tachiom" \
THIRAWAT_QUERY_EMBEDDINGS_PATH="${WORK_DIR}/thirawat-drug/query_embeddings/query_embeddings.json" \
USAGI_API_KEYS="${API_KEY}" \
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

auth_curl \
  -H 'Content-Type: application/json' \
  -d '{
    "idempotency_key": "mapper-batch-smoke-v1",
    "mode": "thirawat_tachiom",
    "candidate_top_k": 10,
    "rerank_top_n": 10,
    "limit": 1,
    "items": [
      {
        "id": "ok-tramadol-50",
        "source_name": "tramadol hydrochloride 50 mg capsule",
        "source_code": "SRC_TRAMADOL_50_CAP"
      },
      {
        "id": "missing-query-embedding",
        "source_name": "query with no precomputed embedding",
        "source_code": "SRC_MISSING"
      }
    ]
  }' \
  "${BASE_URL}/mapper/drugs/batch-job" >"${WORK_DIR}/create-job.json"

JOB_ID="$(python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["state"] == "queued"; print(data["job_id"])' <"${WORK_DIR}/create-job.json")"

JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
JOB_RESULTS_DIR="${WORK_DIR}/results" \
THIRAWAT_ARTIFACT_DIR="${WORK_DIR}/thirawat-drug" \
TACHIOM_INDEX_DIR="${WORK_DIR}/thirawat-drug/tachiom" \
THIRAWAT_QUERY_EMBEDDINGS_PATH="${WORK_DIR}/thirawat-drug/query_embeddings/query_embeddings.json" \
TACHIOM_BACKEND=fixture \
cargo run -q -p api-worker --bin usagi-worker -- --queues map --once >"${WORK_DIR}/api-worker.log"

auth_curl "${BASE_URL}/jobs/${JOB_ID}" >"${WORK_DIR}/job-status.json"
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["state"] == "succeeded_with_errors", data; assert data["processed"] == 2, data; assert data["failed"] == 1, data; assert data["stage"] == "validating_results", data' <"${WORK_DIR}/job-status.json"

auth_curl "${BASE_URL}/jobs/${JOB_ID}/events" >"${WORK_DIR}/job-events.json"
python3 -c 'import json,sys; data=json.load(sys.stdin); stages=[event.get("payload", {}).get("stage") for event in data["events"]]; expected=["validating_indexes","embedding_queries","tachiom_retrieval","bimaxsim_reranking","deterministic_tiebreak","writing_results","validating_results"]; assert stages == expected, stages; print(json.dumps({"stages": stages}))' <"${WORK_DIR}/job-events.json"

python3 -c 'import json,sqlite3,sys; conn=sqlite3.connect(sys.argv[1]); rows=conn.execute("select item_key, state, input_json, result_json, error_json from job_items where job_id = ? order by item_key", (sys.argv[2],)).fetchall(); assert len(rows) == 2, rows; by_key={row[0]: row for row in rows}; ok=by_key["ok-tramadol-50"]; assert ok[1] == "succeeded", ok; assert json.loads(ok[2])["source_name"] == "tramadol hydrochloride 50 mg capsule", ok; assert json.loads(ok[3])["candidates"][0]["concept"]["concept_id"] == 40162522, ok; bad=by_key["missing-query-embedding"]; assert bad[1] == "failed", bad; assert json.loads(bad[2])["source_name"] == "query with no precomputed embedding", bad; assert json.loads(bad[4])["code"] == "EMBEDDING_FAILED", bad; print(json.dumps({"job_items": {"succeeded": 1, "failed": 1}}))' "${WORK_DIR}/jobs/jobs.sqlite" "${JOB_ID}"

auth_curl "${BASE_URL}/jobs/${JOB_ID}/results" >"${WORK_DIR}/job-results.json"
RESULTS_PATH="$(python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["state"] == "succeeded_with_errors", data; artifact=data["artifact"]; assert artifact["content_type"] == "application/jsonl", artifact; assert artifact["provenance"]["model_artifact_id"] == "sidataplus/THIRAWAT-SapBERT", artifact; assert artifact["provenance"]["index_artifact_id"].endswith("/manifest.json"), artifact; print(artifact["path"])' <"${WORK_DIR}/job-results.json")"

python3 -c 'import json,sys; rows=[json.loads(line) for line in open(sys.argv[1], encoding="utf-8") if line.strip()]; assert len(rows) == 2, rows; by_id={row["id"]: row for row in rows}; ok=by_id["ok-tramadol-50"]; assert ok["candidates"][0]["concept"]["concept_id"] == 40162522, ok; missing=by_id["missing-query-embedding"]; assert missing["error"]["code"] == "EMBEDDING_FAILED", missing; manifest=json.load(open(sys.argv[3], encoding="utf-8")); assert manifest["extra"]["provenance"]["model_artifact_id"] == "sidataplus/THIRAWAT-SapBERT", manifest; print(json.dumps({"job_id": sys.argv[2], "state": "succeeded_with_errors", "top_concept_id": ok["candidates"][0]["concept"]["concept_id"], "failed_item_code": missing["error"]["code"], "artifact_provenance": manifest["extra"]["provenance"]}))' "${RESULTS_PATH}" "${JOB_ID}" "${WORK_DIR}/results/${JOB_ID}/manifest.json"
