#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/api-body-limit-smoke"
PORT="${API_BODY_LIMIT_SMOKE_PORT:-8794}"
BASE_URL="http://127.0.0.1:${PORT}"
REQUEST_ID="req_smoke_body_limit"
API_KEY="local-secret"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}"

SEARCH_API_ADDR="127.0.0.1:${PORT}" \
CATALOG_DB_PATH="${WORK_DIR}/catalog.sqlite" \
TANTIVY_INDEX_DIR="${WORK_DIR}/tantivy" \
SAPBERT_INDEX_DIR="${WORK_DIR}/sapbert" \
JOBS_DB_PATH="${WORK_DIR}/jobs.sqlite" \
USAGI_API_KEYS="${API_KEY}" \
USAGI_API_BODY_LIMIT_BYTES=1024 \
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

python3 - "${WORK_DIR}/large-body.json" <<'PY'
import json
import sys

payload = {
    "q": "metformin " + ("x" * 2048),
    "mode": "lexical_tantivy",
    "limit": 1,
}
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    json.dump(payload, handle)
PY

curl -sS \
  -D "${WORK_DIR}/headers.txt" \
  -o "${WORK_DIR}/body.json" \
  -w '%{http_code}' \
  -H "X-Request-Id: ${REQUEST_ID}" \
  -H "X-API-Key: ${API_KEY}" \
  -H 'Content-Type: application/json' \
  --data-binary @"${WORK_DIR}/large-body.json" \
  "${BASE_URL}/search/concepts" >"${WORK_DIR}/status.txt"

python3 - "${REQUEST_ID}" "${WORK_DIR}/headers.txt" "${WORK_DIR}/body.json" "${WORK_DIR}/status.txt" <<'PY'
import json
import sys

expected_request_id, headers_path, body_path, status_path = sys.argv[1:]
status = open(status_path, encoding="utf-8").read().strip()
assert status == "413", status

headers = {}
for line in open(headers_path, encoding="utf-8"):
    if ":" in line:
        key, value = line.split(":", 1)
        headers[key.lower()] = value.strip()
assert headers.get("x-request-id") == expected_request_id, headers

body = json.load(open(body_path, encoding="utf-8"))
assert body["error"]["code"] == "BAD_REQUEST", body
assert body["error"]["request_id"] == expected_request_id, body
print(json.dumps({"status": int(status), "code": "BAD_REQUEST", "request_id": expected_request_id}))
PY
