#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RAILS_BASE_URL="${RAILS_BASE_URL:-http://127.0.0.1:${RAILS_PUBLISH_PORT:-3000}}"
START_STACK="${USAGI_DEPLOY_SMOKE_START:-1}"

COMPOSE_ARGS=(-f "${ROOT_DIR}/infra/docker/docker-compose.yml")
CATALOG_PUBLISH_PORT="${CATALOG_PUBLISH_PORT:-28788}"
SEARCH_PUBLISH_PORT="${SEARCH_PUBLISH_PORT:-28789}"
MAPPER_PUBLISH_PORT="${MAPPER_PUBLISH_PORT:-28790}"
if [[ "${USAGI_DEPLOY_SMOKE_DEBUG_PORTS:-0}" == "1" ]]; then
  COMPOSE_ARGS+=(-f "${ROOT_DIR}/infra/docker/docker-compose.debug.yml")
fi

compose() {
  docker compose "${COMPOSE_ARGS[@]}" "$@"
}

if [[ "${START_STACK}" == "1" ]]; then
  compose up -d --build
fi

for _ in {1..120}; do
  if curl -fsS "${RAILS_BASE_URL}/up" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

curl -fsS "${RAILS_BASE_URL}/up" >/dev/null

compose exec -T rails-web curl -fsS http://catalog-api:8788/catalog/health >/dev/null
compose exec -T rails-web curl -fsS http://search-api:8789/search/health >/dev/null
compose exec -T rails-web curl -fsS http://mapper-api:8790/mapper/health >/dev/null

compose exec -T rails-web bin/rails runner 'require "json"; statuses = { catalog: EngineClients::CatalogClient.new.status.fetch("status"), search: EngineClients::SearchClient.new.status.fetch("status"), mapper: EngineClients::MapperClient.new.status.fetch("status") }; begin; EngineClients::SearchClient.new.search_concepts(q: "metformin", mode: "lexical_tantivy", limit: 1); statuses[:signed_search_probe] = "ok"; rescue EngineClients::BaseClient::Error => e; auth_codes = %w[UNAUTHORIZED SIGNATURE_REQUIRED SIGNATURE_INVALID SIGNATURE_VERSION_UNSUPPORTED SIGNATURE_TIMESTAMP_INVALID SIGNATURE_NONCE_REPLAYED SIGNATURE_BODY_HASH_MISMATCH]; raise if e.status == 401 || auth_codes.include?(e.code); statuses[:signed_search_probe] = e.code; end; puts JSON.generate(statuses)'

if [[ "${USAGI_DEPLOY_SMOKE_RUN_LIVE_TESTS:-0}" == "1" ]]; then
  if [[ "${USAGI_DEPLOY_SMOKE_DEBUG_PORTS:-0}" != "1" ]]; then
    echo "Set USAGI_DEPLOY_SMOKE_DEBUG_PORTS=1 before running host-side live Rails tests." >&2
    exit 1
  fi

  (
    cd "${ROOT_DIR}/apps/web"
    USAGI_LIVE_ENGINE=1 \
    ENGINE_CLIENT_MODE=http \
    USAGI_API_SHARED_SECRET="${USAGI_API_SHARED_SECRET:-local-compose-shared-secret}" \
    CATALOG_API_URL="http://127.0.0.1:${CATALOG_PUBLISH_PORT}" \
    SEARCH_API_URL="http://127.0.0.1:${SEARCH_PUBLISH_PORT}" \
    MAPPER_API_URL="http://127.0.0.1:${MAPPER_PUBLISH_PORT}" \
    JOBS_API_URL="http://127.0.0.1:${MAPPER_PUBLISH_PORT}" \
    bin/rails test test/integration/live_engine_smoke_test.rb test/integration/live_workflow_e2e_test.rb
  )
fi
