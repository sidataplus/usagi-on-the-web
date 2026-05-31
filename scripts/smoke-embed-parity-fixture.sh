#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="${ROOT_DIR}/fixtures/embed-parity"

cargo run -q -p usagi-embed --bin usagi-embed-parity -- \
  cls \
  "${FIXTURE_DIR}/candidate-cls.json" \
  "${FIXTURE_DIR}/reference-cls.json" \
  0.999 |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["status"] == "ok"; assert data["mode"] == "cls"; print(json.dumps(data))'

cargo run -q -p usagi-embed --bin usagi-embed-parity -- \
  token \
  "${FIXTURE_DIR}/candidate-token.json" \
  "${FIXTURE_DIR}/reference-token.json" \
  0.999 \
  96 \
  2 |
python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["status"] == "ok"; assert data["mode"] == "token"; assert data["output_dim"] == 2; print(json.dumps(data))'
