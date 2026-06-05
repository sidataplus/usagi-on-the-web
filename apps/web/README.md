# Usagi Rails App

This is the Rails 8 product workflow layer for Usagi-on-the-Web v3.

Rails owns users, projects, imports, mappings, candidates, comments, audit
events, exports, and engine job mirrors. It calls `usagi-api` only through
`app/services/engine_clients/`.

## Local Setup

```bash
bin/setup
bin/rails server
```

Development and normal tests use stubbed engine clients unless
`ENGINE_CLIENT_MODE=http` is set.

## Test And Lint

```bash
bin/rails test
bin/rails test:system
bin/rubocop
bin/brakeman --no-pager
```

## Live Engine Tests

Start the API stack first, then run:

```bash
USAGI_LIVE_ENGINE=1 \
ENGINE_CLIENT_MODE=http \
USAGI_API_SHARED_SECRET=local-compose-shared-secret \
CATALOG_API_URL=http://127.0.0.1:28788 \
SEARCH_API_URL=http://127.0.0.1:28789 \
MAPPER_API_URL=http://127.0.0.1:28790 \
JOBS_API_URL=http://127.0.0.1:28790 \
bin/rails test test/integration/live_engine_smoke_test.rb
```

Use the full-stack Compose debug override when host-side tests need direct
engine URLs.

## Production Env

Required production variables:

```text
SECRET_KEY_BASE
DATABASE_URL
USAGI_API_SHARED_SECRET
CATALOG_API_URL
SEARCH_API_URL
MAPPER_API_URL
JOBS_API_URL
APP_HOSTS
```

Set `FORCE_SSL=false` only for local production-mode Compose. Production Kamal
deployments should leave SSL enabled.

## Deployment Docs

See:

```text
../../docs/manual/web/04-deployment.md
../../docs/security/rails-api-boundary.md
../../infra/kamal/README.md
../../infra/aws/README.md
../../infra/modal/README.md
```
