#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/request-id-smoke"
PORT="${REQUEST_ID_SMOKE_PORT:-8792}"
BASE_URL="http://127.0.0.1:${PORT}"
REQUEST_ID="req_smoke_request_id"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}"

SEARCH_API_ADDR="127.0.0.1:${PORT}" \
CATALOG_DB_PATH="${WORK_DIR}/catalog.sqlite" \
TANTIVY_INDEX_DIR="${WORK_DIR}/tantivy" \
SAPBERT_INDEX_DIR="${WORK_DIR}/sapbert" \
JOBS_DB_PATH="${WORK_DIR}/jobs.sqlite" \
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

STATUS_FILE="${WORK_DIR}/status.txt"
BODY_FILE="${WORK_DIR}/body.json"
HEADER_FILE="${WORK_DIR}/headers.txt"

curl -sS \
  -D "${HEADER_FILE}" \
  -o "${BODY_FILE}" \
  -w '%{http_code}' \
  -H "X-Request-Id: ${REQUEST_ID}" \
  -H 'Content-Type: application/json' \
  -d '{"q": "metformin", "mode": "lexical_tantivy", "limit": 1}' \
  "${BASE_URL}/search/concepts" >"${STATUS_FILE}"

python3 - "${REQUEST_ID}" "${STATUS_FILE}" "${HEADER_FILE}" "${BODY_FILE}" <<'PY'
import json
import sys

expected, status_path, header_path, body_path = sys.argv[1:]
status = open(status_path, encoding="utf-8").read().strip()
assert status == "500", status

headers = {}
for line in open(header_path, encoding="utf-8"):
    if ":" in line:
        key, value = line.split(":", 1)
        headers[key.lower()] = value.strip()
assert headers.get("x-request-id") == expected, headers

body = json.load(open(body_path, encoding="utf-8"))
assert body["error"]["request_id"] == expected, body
print(json.dumps({
    "case": "service_error",
    "status": int(status),
    "request_id": body["error"]["request_id"],
    "code": body["error"]["code"]
}))
PY

MALFORMED_STATUS_FILE="${WORK_DIR}/malformed-status.txt"
MALFORMED_BODY_FILE="${WORK_DIR}/malformed-body.json"
MALFORMED_HEADER_FILE="${WORK_DIR}/malformed-headers.txt"

curl -sS \
  -D "${MALFORMED_HEADER_FILE}" \
  -o "${MALFORMED_BODY_FILE}" \
  -w '%{http_code}' \
  -H "X-Request-Id: ${REQUEST_ID}" \
  -H 'Content-Type: application/json' \
  -d '{"q":' \
  "${BASE_URL}/search/concepts" >"${MALFORMED_STATUS_FILE}"

python3 - "${REQUEST_ID}" "${MALFORMED_STATUS_FILE}" "${MALFORMED_HEADER_FILE}" "${MALFORMED_BODY_FILE}" <<'PY'
import json
import sys

expected, status_path, header_path, body_path = sys.argv[1:]
status = open(status_path, encoding="utf-8").read().strip()
assert status in {"400", "422"}, status

headers = {}
for line in open(header_path, encoding="utf-8"):
    if ":" in line:
        key, value = line.split(":", 1)
        headers[key.lower()] = value.strip()
assert headers.get("x-request-id") == expected, headers

body = json.load(open(body_path, encoding="utf-8"))
assert body["error"]["code"] == "BAD_REQUEST", body
assert body["error"]["request_id"] == expected, body
print(json.dumps({
    "case": "malformed_json",
    "status": int(status),
    "request_id": body["error"]["request_id"],
    "code": body["error"]["code"]
}))
PY
