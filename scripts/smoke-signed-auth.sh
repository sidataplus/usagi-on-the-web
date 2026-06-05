#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/signed-auth-smoke"
PORT="${SIGNED_AUTH_SMOKE_PORT:-8794}"
BASE_URL="http://127.0.0.1:${PORT}"
REQUEST_ID_PREFIX="req_smoke_signed"
SHARED_SECRET="${USAGI_API_SHARED_SECRET:-signed-smoke-secret}"
BODY='{"q":"metformin","mode":"lexical_tantivy","limit":1}'

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}"

SEARCH_API_ADDR="127.0.0.1:${PORT}" \
CATALOG_DB_PATH="${WORK_DIR}/catalog.sqlite" \
TANTIVY_INDEX_DIR="${WORK_DIR}/tantivy" \
SAPBERT_INDEX_DIR="${WORK_DIR}/sapbert" \
JOBS_DB_PATH="${WORK_DIR}/jobs.sqlite" \
USAGI_API_AUTH_MODE=signed \
USAGI_API_SHARED_SECRET="${SHARED_SECRET}" \
CARGO_TARGET_DIR="${USAGI_CARGO_TARGET_DIR:-${ROOT_DIR}/target}" \
cargo run -q -p search-api >"${WORK_DIR}/search-api.log" 2>&1 &
SERVER_PID=$!
trap 'kill "${SERVER_PID}" 2>/dev/null || true' EXIT

for _ in {1..240}; do
  if curl -fsS "${BASE_URL}/search/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

curl -fsS "${BASE_URL}/search/health" >/dev/null
curl -fsS "${BASE_URL}/search/status" >/dev/null

assert_error_response() {
  local expected_status="$1"
  local expected_code="$2"
  local headers_file="$3"
  local body_file="$4"
  local status_file="$5"

  python3 - "${expected_status}" "${expected_code}" "${headers_file}" "${body_file}" "${status_file}" <<'PY'
import json
import sys

expected_status, expected_code, headers_path, body_path, status_path = sys.argv[1:]
status = open(status_path, encoding="utf-8").read().strip()
assert status == expected_status, status

headers = {}
for line in open(headers_path, encoding="utf-8"):
    if ":" in line:
        key, value = line.split(":", 1)
        headers[key.lower()] = value.strip()
assert "x-request-id" in headers, headers

body = json.load(open(body_path, encoding="utf-8"))
assert body["error"]["code"] == expected_code, body
assert body["error"]["request_id"] == headers["x-request-id"], body
print(json.dumps({"status": int(status), "code": expected_code, "request_id": headers["x-request-id"]}))
PY
}

signed_headers() {
  local method="$1"
  local path="$2"
  local body="$3"
  local request_id="$4"

  python3 - "${method}" "${path}" "${body}" "${request_id}" "${SHARED_SECRET}" <<'PY'
import base64
import datetime as dt
import hashlib
import hmac
import secrets
import sys

method, path, body, request_id, secret = sys.argv[1:]
timestamp = dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
nonce = f"nonce_{secrets.token_hex(16)}"
content_sha = hashlib.sha256(body.encode("utf-8")).hexdigest()
canonical = "\n".join([method.upper(), path, timestamp, nonce, request_id, content_sha])
signature = base64.b64encode(hmac.new(secret.encode("utf-8"), canonical.encode("utf-8"), hashlib.sha256).digest()).decode("ascii")

for header in [
    f"X-Request-Id: {request_id}",
    "X-Usagi-Service: rails",
    f"X-Usagi-Timestamp: {timestamp}",
    f"X-Usagi-Nonce: {nonce}",
    f"X-Usagi-Content-SHA256: {content_sha}",
    f"X-Usagi-Signature: v1={signature}",
]:
    print(header)
PY
}

curl -sS \
  -D "${WORK_DIR}/missing-signature-headers.txt" \
  -o "${WORK_DIR}/missing-signature-body.json" \
  -w '%{http_code}' \
  -H "X-Request-Id: ${REQUEST_ID_PREFIX}_missing" \
  -H 'Content-Type: application/json' \
  -d "${BODY}" \
  "${BASE_URL}/search/concepts" >"${WORK_DIR}/missing-signature-status.txt"
assert_error_response 401 SIGNATURE_REQUIRED "${WORK_DIR}/missing-signature-headers.txt" "${WORK_DIR}/missing-signature-body.json" "${WORK_DIR}/missing-signature-status.txt"

SIGNED_CURL_ARGS=()
while IFS= read -r header; do
  SIGNED_CURL_ARGS+=("-H" "${header}")
done < <(signed_headers POST /search/concepts "${BODY}" "${REQUEST_ID_PREFIX}_valid")

curl -sS \
  -D "${WORK_DIR}/valid-signature-headers.txt" \
  -o "${WORK_DIR}/valid-signature-body.json" \
  -w '%{http_code}' \
  -H 'Content-Type: application/json' \
  "${SIGNED_CURL_ARGS[@]}" \
  -d "${BODY}" \
  "${BASE_URL}/search/concepts" >"${WORK_DIR}/valid-signature-status.txt"

python3 - "${WORK_DIR}/valid-signature-body.json" "${WORK_DIR}/valid-signature-status.txt" <<'PY'
import json
import sys

body_path, status_path = sys.argv[1:]
status = int(open(status_path, encoding="utf-8").read().strip())
assert status != 401, status

body = json.load(open(body_path, encoding="utf-8"))
code = body.get("error", {}).get("code")
assert code not in {
    "UNAUTHORIZED",
    "SIGNATURE_REQUIRED",
    "SIGNATURE_INVALID",
    "SIGNATURE_VERSION_UNSUPPORTED",
    "SIGNATURE_TIMESTAMP_INVALID",
    "SIGNATURE_NONCE_REPLAYED",
    "SIGNATURE_BODY_HASH_MISMATCH",
}, body
print(json.dumps({"status": status, "auth": "signed request accepted", "engine_code": code}))
PY
