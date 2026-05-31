# Rails Verification Manual

Status: draft v0.1  
Audience: Rails developers and reviewers

## Offline Verification

Run:

```bash
cd apps/web
bin/rails test
```

The suite uses:

```text
in-process engine stubs
fixtures/contracts JSON responses
request/model/service tests
gated live smoke tests
```

Normal Rails tests must pass without running `usagi-api`.

## Live API Smoke

Start the API stack first, then run:

```bash
cd apps/web
USAGI_LIVE_ENGINE=1 \
ENGINE_CLIENT_MODE=http \
USAGI_API_KEY=dev-secret \
USAGI_API_SHARED_SECRET=dev-secret \
CATALOG_API_URL=http://127.0.0.1:8788 \
SEARCH_API_URL=http://127.0.0.1:8789 \
MAPPER_API_URL=http://127.0.0.1:8790 \
bin/rails test test/integration/live_engine_smoke_test.rb
```

The smoke test checks readiness endpoints only. It is intentionally gated so CI
and local Rails work do not accidentally depend on a live distributed stack.

## Done Criteria

A Rails slice is ready when:

```text
bin/rails test passes
request signing remains covered
contract fixture parsing remains covered
no browser-to-engine shortcut exists
Rails does not read engine artifacts
manual candidate approval remains explicit
```
