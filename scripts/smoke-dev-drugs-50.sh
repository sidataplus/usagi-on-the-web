#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${ROOT_DIR}/temp/dev-drugs-50-smoke"
PORT="${DEV_DRUGS_50_SMOKE_PORT:-8793}"
BASE_URL="http://127.0.0.1:${PORT}"
FIXTURE_DIR="${ROOT_DIR}/fixtures/dev-drugs-50"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}/thirawat-drug"

cargo run -q -p usagi-thirawat --bin usagi-dev-drugs-50-fixture -- \
  --fixture-dir "${FIXTURE_DIR}" \
  --artifact-dir "${WORK_DIR}/thirawat-drug"

THIRAWAT_ARTIFACT_DIR="${WORK_DIR}/thirawat-drug" \
THIRAWAT_DOC_EMBEDDING_DIR="${WORK_DIR}/thirawat-drug/doc_embeddings" \
TACHIOM_INDEX_DIR="${WORK_DIR}/thirawat-drug/tachiom" \
TACHIOM_ARTIFACT_ID="fixture-dev-drugs-50-tachiom-v1" \
cargo run -q -p usagi-tachiom --bin usagi-tachiom-build >/dev/null

MAPPER_API_ADDR="127.0.0.1:${PORT}" \
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

python3 - "${FIXTURE_DIR}/source_terms.csv" "${BASE_URL}/mapper/drugs/batch" <<'PY'
import csv
import json
import sys
import urllib.request

source_terms_path, endpoint = sys.argv[1:3]
with open(source_terms_path, newline="", encoding="utf-8") as handle:
    rows = list(csv.DictReader(handle))

payload = {
    "mode": "thirawat_tachiom",
    "candidate_top_k": 50,
    "rerank_top_n": 50,
    "limit": 20,
    "items": [
        {
            "id": row["source_code"],
            "source_name": row["source_name"],
            "source_code": row["source_code"],
            "source_frequency": int(row["source_frequency"]),
        }
        for row in rows
    ],
}
request = urllib.request.Request(
    endpoint,
    data=json.dumps(payload).encode("utf-8"),
    headers={"Content-Type": "application/json"},
    method="POST",
)
with urllib.request.urlopen(request, timeout=30) as response:
    data = json.load(response)

expected_by_id = {
    row["source_code"]: int(row["expected_target_concept_id"])
    for row in rows
}
missing = []
for item in data["items"]:
    expected = expected_by_id[item["id"]]
    candidate_ids = [candidate["concept"]["concept_id"] for candidate in item["candidates"]]
    if expected not in candidate_ids:
        missing.append({
            "id": item["id"],
            "expected": expected,
            "candidate_ids": candidate_ids,
            "error": item.get("error"),
        })

assert not missing, json.dumps(missing[:5], indent=2)
print(json.dumps({
    "checked_items": len(data["items"]),
    "checked_targets": len(set(expected_by_id.values())),
    "status": "all_expected_targets_in_candidates",
}))
PY
