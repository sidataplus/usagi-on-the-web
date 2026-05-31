#!/usr/bin/env bash
set -euo pipefail

CATALOG_BASE_URL="${CATALOG_BASE_URL:-http://127.0.0.1:8788}"
SEARCH_BASE_URL="${SEARCH_BASE_URL:-http://127.0.0.1:8789}"
MAPPER_BASE_URL="${MAPPER_BASE_URL:-http://127.0.0.1:8790}"
API_KEY="${USAGI_SMOKE_API_KEY:-smoke-secret}"
RUN_ID="${USAGI_SMOKE_RUN_ID:-$(date +%s)}"

auth_curl() {
  curl -fsS -H "X-API-Key: ${API_KEY}" "$@"
}

poll_job() {
  local base_url="$1"
  local job_id="$2"
  local expected_state="$3"
  local out_path="$4"

  for _ in {1..60}; do
    auth_curl "${base_url}/jobs/${job_id}" >"${out_path}"
    if python3 - "${out_path}" "${expected_state}" <<'PY'
import json
import sys

path, expected = sys.argv[1:3]
with open(path, encoding="utf-8") as handle:
    data = json.load(handle)
state = data["state"]
if state == expected:
    sys.exit(0)
if state in {"failed", "cancelled", "succeeded_with_errors"}:
    raise SystemExit(f"job ended in unexpected state: {data}")
raise SystemExit(1)
PY
    then
      return 0
    fi
    sleep 1
  done
  echo "timed out waiting for job ${job_id} to reach ${expected_state}" >&2
  cat "${out_path}" >&2
  return 1
}

job_id_from() {
  python3 -c 'import json,sys; data=json.load(sys.stdin); print(data["job_id"])'
}

curl -fsS "${CATALOG_BASE_URL}/catalog/health"
curl -fsS "${SEARCH_BASE_URL}/search/health"
curl -fsS "${MAPPER_BASE_URL}/mapper/health"

curl -fsS "${CATALOG_BASE_URL}/catalog/status"
curl -fsS "${SEARCH_BASE_URL}/search/status"
curl -fsS "${MAPPER_BASE_URL}/mapper/status"

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/usagi-api-smoke.XXXXXX")"
trap 'rm -rf "${WORK_DIR}"' EXIT

auth_curl \
  -H 'Content-Type: application/json' \
  -d "{\"athena_dir\":\"/fixtures/search-smoke/athena-mini\",\"idempotency_key\":\"compose-catalog-${RUN_ID}\"}" \
  "${CATALOG_BASE_URL}/catalog/build-job" >"${WORK_DIR}/catalog-job.json"
CATALOG_JOB_ID="$(job_id_from <"${WORK_DIR}/catalog-job.json")"
poll_job "${CATALOG_BASE_URL}" "${CATALOG_JOB_ID}" "succeeded" "${WORK_DIR}/catalog-status.json"

auth_curl \
  -H 'Content-Type: application/json' \
  -d "{\"idempotency_key\":\"compose-tantivy-${RUN_ID}\"}" \
  "${SEARCH_BASE_URL}/search/tantivy/build-job" >"${WORK_DIR}/tantivy-job.json"
TANTIVY_JOB_ID="$(job_id_from <"${WORK_DIR}/tantivy-job.json")"
poll_job "${SEARCH_BASE_URL}" "${TANTIVY_JOB_ID}" "succeeded" "${WORK_DIR}/tantivy-status.json"

auth_curl \
  -H 'Content-Type: application/json' \
  -d '{"q":"tramadol 50 mg capsule","mode":"lexical_tantivy","limit":1}' \
  "${SEARCH_BASE_URL}/search/concepts" |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["results"][0]["concept"]["concept_id"] == 100, data; print(json.dumps({"lexical_top_concept_id": 100}))'
