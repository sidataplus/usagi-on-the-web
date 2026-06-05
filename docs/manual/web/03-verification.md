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

## Deployment Smoke

Validate signed engine auth:

```bash
scripts/smoke-signed-auth.sh
```

Validate the full local deployment topology:

```bash
scripts/smoke-deploy.sh
```

The deployment smoke starts the full-stack Compose file by default, checks
Rails `/up` from the host, checks engine health from inside the Rails container,
and asks Rails engine clients to call the private API services with signed
headers.

## Live API Smoke

### Full-stack compose (signed, recommended)

Start the full local deployment stack:

```bash
docker compose -f infra/docker/docker-compose.yml up -d --build
scripts/smoke-deploy.sh
```

For host-side live Rails tests, enable the debug override so engine services are
reachable from the host on non-conflicting ports:

```bash
USAGI_DEPLOY_SMOKE_START=0 \
USAGI_DEPLOY_SMOKE_DEBUG_PORTS=1 \
USAGI_DEPLOY_SMOKE_RUN_LIVE_TESTS=1 \
scripts/smoke-deploy.sh
```

The debug override publishes `127.0.0.1:28788-28790` by default. Use
`USAGI_API_SHARED_SECRET=local-compose-shared-secret` unless you changed the
compose env.

The Rails live smoke checks:

```text
catalog/search/mapper status clients
search concept DTO parsing
search batch item id and provenance
mapper single-query DTO parsing
mapper batch-job creation, polling, and JSONL result streaming
```

It is intentionally gated so CI and local Rails work do not accidentally depend
on a live distributed stack.

### API-only compose (api_key, development)

For API-only development against `docker-compose.api.yml`:

```bash
USAGI_API_KEYS=smoke-secret \
docker compose -f infra/docker/docker-compose.api.yml up -d --build
USAGI_SMOKE_API_KEY=smoke-secret scripts/smoke-api.sh
```

That path uses `USAGI_API_KEY` rather than signed requests.

## Live Rails Workflow E2E

After the client-level smoke passes, run the Rails workflow proof against the
signed full-stack compose file:

```bash
USAGI_DEPLOY_SMOKE_START=0 \
USAGI_DEPLOY_SMOKE_DEBUG_PORTS=1 \
USAGI_DEPLOY_SMOKE_RUN_LIVE_TESTS=1 \
scripts/smoke-deploy.sh
```

This proof creates projects through Rails, imports source terms, runs manual
search, starts the THIRAWAT drug mapper batch job, polls and persists mapper
JSONL results, runs hybrid auto-suggest for a mixed project, and verifies the
job and review pages render both success and partial-failure states.

The test intentionally covers two production seams:

```text
mapper JSONL rows with item-level errors are copied into engine_jobs.error.items
Rails-local hybrid job mirrors render from local state when no API job id exists
```

## Browser Workflow Check

When the in-app Browser is available, run the same product path through the
rendered Rails UI before claiming the workflow is flawless end to end.

Start Rails with the live API environment while the signed compose stack is
running with debug ports:

```bash
cd apps/web
ENGINE_CLIENT_MODE=http \
USAGI_API_SHARED_SECRET=local-compose-shared-secret \
CATALOG_API_URL=http://127.0.0.1:28788 \
SEARCH_API_URL=http://127.0.0.1:28789 \
MAPPER_API_URL=http://127.0.0.1:28790 \
JOBS_API_URL=http://127.0.0.1:28790 \
bin/rails server -p 3220 -b 127.0.0.1
```

Then use Browser against `http://127.0.0.1:3220`:

```text
sign in as demo@usagi.test / password123
create a Drug project
import source terms with one mappable drug and one expected mapper failure
run manual search and verify a candidate appears
run Drug auto-suggest and open the job detail page
verify partial failures show source code, error code, and retry
create a Mixed project with a Drug source_domain_hint row
run auto-suggest and open the local hybrid job detail page
verify the job succeeds and the review page shows persisted candidates
```

The checklist can also be run as a Browser harness from the in-app Browser
runtime after selecting the `iab` browser and current tab:

```js
const { runLiveBrowserWorkflow } = await import(
  "/Users/na399/GitHub/sidataplus/usagi-on-the-web/scripts/browser-live-workflow.mjs"
);

const result = await runLiveBrowserWorkflow({
  browser,
  tab,
  baseUrl: "http://127.0.0.1:3220",
  captureScreenshots: false
});

console.log(JSON.stringify(result, null, 2));
```

The harness drives the rendered Rails pages only. It should not call engine APIs
directly, and it should return `ok: true` only after the job pages and Review
workspace expose the expected success and failure states.

Use live fixture-backed terms for the happy paths:

```text
Drug mapper success: tramadol hydrochloride 50 mg capsule
Drug mapper expected failure: query with no precomputed embedding
Hybrid success: tramadol 50 mg capsule
```

If a hybrid batch fails before processing rows, the job detail page should show
the failed state, preserve the engine error code and request ID when available,
and count the unprocessed rows as failed rows.

If Browser cannot attach to an in-app tab, record that as a Browser harness
blocker and keep the live Rails workflow test output with the run notes. Do not
substitute browser-to-engine calls or a non-Browser automation surface without an
explicit reviewer decision.

## Done Criteria

A Rails slice is ready when:

```text
bin/rails test passes
request signing remains covered
contract fixture parsing remains covered
live engine smoke passes before claiming client-level API readiness
live Rails workflow E2E passes before claiming product workflow readiness
Browser workflow check passes before claiming rendered end-to-end readiness
no browser-to-engine shortcut exists
Rails does not read engine artifacts
manual candidate approval remains explicit
```
