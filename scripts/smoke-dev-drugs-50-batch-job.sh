#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/dev-drugs-50-batch-job-smoke"
PORT="${DEV_DRUGS_50_BATCH_SMOKE_PORT:-8794}"
BASE_URL="http://127.0.0.1:${PORT}"
FIXTURE_DIR="${ROOT_DIR}/fixtures/dev-drugs-50"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/thirawat-drug" "${WORK_DIR}/jobs" "${WORK_DIR}/results"

cargo run -q -p usagi-thirawat --bin usagi-dev-drugs-50-fixture -- \
  --fixture-dir "${FIXTURE_DIR}" \
  --artifact-dir "${WORK_DIR}/thirawat-drug"

THIRAWAT_ARTIFACT_DIR="${WORK_DIR}/thirawat-drug" \
THIRAWAT_DOC_EMBEDDING_DIR="${WORK_DIR}/thirawat-drug/doc_embeddings" \
TACHIOM_INDEX_DIR="${WORK_DIR}/thirawat-drug/tachiom" \
TACHIOM_ARTIFACT_ID="fixture-dev-drugs-50-tachiom-v1" \
TACHIOM_BACKEND=fixture \
cargo run -q -p usagi-tachiom --bin usagi-tachiom-build >/dev/null

MAPPER_API_ADDR="127.0.0.1:${PORT}" \
JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
JOB_RESULTS_DIR="${WORK_DIR}/results" \
THIRAWAT_ARTIFACT_DIR="${WORK_DIR}/thirawat-drug" \
TACHIOM_INDEX_DIR="${WORK_DIR}/thirawat-drug/tachiom" \
THIRAWAT_QUERY_EMBEDDINGS_PATH="${WORK_DIR}/thirawat-drug/query_embeddings/query_embeddings.json" \
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

python3 - "${FIXTURE_DIR}/source_terms.csv" "${BASE_URL}/mapper/drugs/batch-job" "${WORK_DIR}/expected.json" <<'PY' >"${WORK_DIR}/create-job.json"
import csv
import json
import sys
import urllib.request

source_terms_path, endpoint, expected_path = sys.argv[1:4]
with open(source_terms_path, newline="", encoding="utf-8") as handle:
    source_rows = list(csv.DictReader(handle))

items = []
expected_by_id = {}
for index in range(500):
    row = source_rows[index % len(source_rows)]
    item_id = f'{row["source_code"]}__{index:03d}'
    items.append({
        "id": item_id,
        "source_name": row["source_name"],
        "source_code": row["source_code"],
        "source_frequency": int(row["source_frequency"]),
    })
    expected_by_id[item_id] = int(row["expected_target_concept_id"])

with open(expected_path, "w", encoding="utf-8") as handle:
    json.dump(expected_by_id, handle)

payload = {
    "idempotency_key": "dev-drugs-50-500-batch-v1",
    "mode": "thirawat_tachiom",
    "candidate_top_k": 50,
    "rerank_top_n": 50,
    "limit": 20,
    "items": items,
}
request = urllib.request.Request(
    endpoint,
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"},
    method="POST",
)
with urllib.request.urlopen(request, timeout=30) as response:
    sys.stdout.write(response.read().decode("utf-8"))
PY

JOB_ID="$(python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["state"] == "queued", data; print(data["job_id"])' <"${WORK_DIR}/create-job.json")"

JOBS_DB_PATH="${WORK_DIR}/jobs/jobs.sqlite" \
JOB_RESULTS_DIR="${WORK_DIR}/results" \
THIRAWAT_ARTIFACT_DIR="${WORK_DIR}/thirawat-drug" \
TACHIOM_INDEX_DIR="${WORK_DIR}/thirawat-drug/tachiom" \
THIRAWAT_QUERY_EMBEDDINGS_PATH="${WORK_DIR}/thirawat-drug/query_embeddings/query_embeddings.json" \
TACHIOM_BACKEND=fixture \
cargo run -q -p api-worker --bin usagi-worker -- --queues map --once >"${WORK_DIR}/api-worker.log"

curl -fsS "${BASE_URL}/jobs/${JOB_ID}" >"${WORK_DIR}/job-status.json"
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["state"] == "succeeded", data; assert data["processed"] == 500, data; assert data["failed"] == 0, data' <"${WORK_DIR}/job-status.json"

curl -fsS "${BASE_URL}/jobs/${JOB_ID}/results" >"${WORK_DIR}/job-results.json"
RESULTS_PATH="$(python3 -c 'import json,sys; data=json.load(sys.stdin); artifact=data["artifact"]; assert data["state"] == "succeeded", data; assert artifact["content_type"] == "application/jsonl", artifact; print(artifact["path"])' <"${WORK_DIR}/job-results.json")"

python3 - "${RESULTS_PATH}" "${WORK_DIR}/expected.json" "${WORK_DIR}/jobs/jobs.sqlite" "${JOB_ID}" <<'PY'
import json
import sqlite3
import sys

results_path, expected_path, db_path, job_id = sys.argv[1:5]
with open(expected_path, encoding="utf-8") as handle:
    expected_by_id = json.load(handle)
with open(results_path, encoding="utf-8") as handle:
    rows = [json.loads(line) for line in handle if line.strip()]

assert len(rows) == 500, len(rows)
missing = []
for row in rows:
    expected = expected_by_id[row["id"]]
    candidate_ids = [candidate["concept"]["concept_id"] for candidate in row["candidates"]]
    if expected not in candidate_ids:
        missing.append({"id": row["id"], "expected": expected, "candidate_ids": candidate_ids})
assert not missing, missing[:5]

conn = sqlite3.connect(db_path)
succeeded, failed = conn.execute(
    "select sum(state = 'succeeded'), sum(state = 'failed') from job_items where job_id = ?",
    (job_id,),
).fetchone()
assert succeeded == 500, succeeded
assert failed in (0, None), failed
print(json.dumps({
    "job_id": job_id,
    "checked_items": len(rows),
    "job_items_succeeded": succeeded,
    "status": "async_500_source_fixture_passed",
}))
PY
