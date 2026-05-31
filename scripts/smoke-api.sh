#!/usr/bin/env bash
set -euo pipefail

curl -fsS localhost:8788/catalog/health
curl -fsS localhost:8789/search/health
curl -fsS localhost:8790/mapper/health

curl -fsS localhost:8788/catalog/status
curl -fsS localhost:8789/search/status
curl -fsS localhost:8790/mapper/status

