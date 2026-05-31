#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/api-key-smoke"
PORT="${API_KEY_SMOKE_PORT:-8793}"
BASE_URL="http://127.0.0.1:${PORT}"
REQUEST_ID="req_smoke_api_key"
API_KEY="local-secret"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}"

SEARCH_API_ADDR="127.0.0.1:${PORT}" \
CATALOG_DB_PATH="${WORK_DIR}/catalog.sqlite" \
TANTIVY_INDEX_DIR="${WORK_DIR}/tantivy" \
SAPBERT_INDEX_DIR="${WORK_DIR}/sapbert" \
JOBS_DB_PATH="${WORK_DIR}/jobs.sqlite" \
USAGI_API_KEYS="${API_KEY}" \
cargo run -q -p search-api >"${WORK_DIR}/search-api.log" 2>&1 &
SERVER_PID=$!
trap 'kill "${SERVER_PID}" 2>/dev/null || true' EXIT

for _ in {1..100}; do
  if curl -fsS "${BASE_URL}/search/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

curl -fsS "${BASE_URL}/search/health" >/dev/null
curl -fsS "${BASE_URL}/search/status" >/dev/null

assert_response() {
  local expected_status="$1"
  local expected_code="$2"
  local headers_file="$3"
  local body_file="$4"
  local status_file="$5"

  python3 - "${expected_status}" "${expected_code}" "${REQUEST_ID}" "${headers_file}" "${body_file}" "${status_file}" <<'PY'
import json
import sys

expected_status, expected_code, expected_request_id, headers_path, body_path, status_path = sys.argv[1:]
status = open(status_path, encoding="utf-8").read().strip()
assert status == expected_status, status

headers = {}
for line in open(headers_path, encoding="utf-8"):
    if ":" in line:
        key, value = line.split(":", 1)
        headers[key.lower()] = value.strip()
assert headers.get("x-request-id") == expected_request_id, headers

body = json.load(open(body_path, encoding="utf-8"))
assert body["error"]["code"] == expected_code, body
assert body["error"]["request_id"] == expected_request_id, body
print(json.dumps({"status": int(status), "code": expected_code, "request_id": expected_request_id}))
PY
}

curl -sS \
  -D "${WORK_DIR}/missing-key-headers.txt" \
  -o "${WORK_DIR}/missing-key-body.json" \
  -w '%{http_code}' \
  -H "X-Request-Id: ${REQUEST_ID}" \
  -H 'Content-Type: application/json' \
  -d '{"q":"metformin","mode":"lexical_tantivy","limit":1}' \
  "${BASE_URL}/search/concepts" >"${WORK_DIR}/missing-key-status.txt"
assert_response 401 UNAUTHORIZED "${WORK_DIR}/missing-key-headers.txt" "${WORK_DIR}/missing-key-body.json" "${WORK_DIR}/missing-key-status.txt"

curl -sS \
  -D "${WORK_DIR}/wrong-key-headers.txt" \
  -o "${WORK_DIR}/wrong-key-body.json" \
  -w '%{http_code}' \
  -H "X-Request-Id: ${REQUEST_ID}" \
  -H 'X-API-Key: wrong' \
  -H 'Content-Type: application/json' \
  -d '{"q":"metformin","mode":"lexical_tantivy","limit":1}' \
  "${BASE_URL}/search/concepts" >"${WORK_DIR}/wrong-key-status.txt"
assert_response 401 UNAUTHORIZED "${WORK_DIR}/wrong-key-headers.txt" "${WORK_DIR}/wrong-key-body.json" "${WORK_DIR}/wrong-key-status.txt"

curl -sS \
  -D "${WORK_DIR}/valid-key-headers.txt" \
  -o "${WORK_DIR}/valid-key-body.json" \
  -w '%{http_code}' \
  -H "X-Request-Id: ${REQUEST_ID}" \
  -H "Authorization: Bearer ${API_KEY}" \
  -H 'Content-Type: application/json' \
  -d '{"q":"metformin","mode":"lexical_tantivy","limit":1}' \
  "${BASE_URL}/search/concepts" >"${WORK_DIR}/valid-key-status.txt"
assert_response 500 INTERNAL_ERROR "${WORK_DIR}/valid-key-headers.txt" "${WORK_DIR}/valid-key-body.json" "${WORK_DIR}/valid-key-status.txt"
