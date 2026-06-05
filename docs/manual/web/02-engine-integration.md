# Rails Engine Integration Manual

Status: draft v0.1  
Audience: Rails developers and API integrators

## Runtime Configuration

| Variable | Default | Meaning |
|---|---|---|
| `ENGINE_CLIENT_MODE` | `stub` | Use in-process stubs unless set to `http` |
| `CATALOG_API_URL` | required for HTTP mode | `catalog-api` base URL |
| `SEARCH_API_URL` | required for HTTP mode | `search-api` base URL |
| `MAPPER_API_URL` | required for HTTP mode | `mapper-api` base URL |
| `JOBS_API_URL` | falls back to `MAPPER_API_URL` | Jobs endpoint base URL |
| `USAGI_API_KEY` | unset | Optional API key sent as `X-API-Key` |
| `USAGI_API_BEARER_TOKEN` | unset | Optional bearer token |
| `USAGI_API_SHARED_SECRET` | `test-secret` | HMAC request signing secret |
| `ENGINE_API_TIMEOUT_SECONDS` | `30` | HTTP read timeout |
| `ENGINE_API_OPEN_TIMEOUT_SECONDS` | `5` | HTTP connection timeout |
| `ENGINE_API_GET_RETRIES` | `1` | Transient retry count for safe GET requests |
| `ENGINE_API_JOB_POLL_INTERVAL_SECONDS` | `2` | Async job polling delay |

## Client Rules

All clients inherit from `EngineClients::BaseClient`.

Required behavior:

```text
request ID propagation
API key or bearer auth when configured
signed request headers outside test mode
JSON request/response handling
standard error envelope parsing
timeouts
safe GET retries only
structured request and retry logging
idempotency keys for job creation
safe stub mode for offline tests
```

`GET` and JSONL result fetches may retry transient connection errors such as
timeouts, refused connections, resets, EOF, and DNS/socket failures. `POST`
requests are not retried by the HTTP transport; job creation safety comes from
the API idempotency key and the Rails mirror reuse rules.

Rails logs `engine.request` for completed engine HTTP calls and
`engine.request.retry` before a safe retry. These logs include method, path,
status when available, `X-Request-Id`, attempt, and duration or error class.
They must not include secrets, signatures, large request bodies, or raw result
artifacts.

## Service Families

Rails uses:

```text
CatalogClient
SearchClient
MapperClient
JobsClient
```

Drug projects use `mapper-api`. Non-drug and mixed projects use hybrid search.
For mixed projects, Rails sends the source term's domain hint as the per-item
`domain_id` filter instead of the project-level `"Mixed"` label.

## Job Mirrors

Rails stores an `engine_jobs` row for every project-level engine workflow. The
API job ID is a mirror of `usagi-api` state, not a license to inspect
`jobs.sqlite`. Mapper batch jobs have an API job ID after creation. Rails-local
workflows, including manual search and hybrid batch search, may have no API job
ID and must render progress from the local mirror.

Mapper batch mirrors store the request idempotency key in `engine_jobs.input`.
A clean `succeeded` mirror is reusable. A mirror with an API job ID in `queued`,
`starting`, or `running` is reused and polled. Local `failed` mirrors and
`succeeded_with_errors` mirrors are retryable with the same idempotency key;
the retry clears stale API URLs/results before calling the mapper API again.

If mapper job creation fails before an API job ID exists, Rails still records a
failed mirror with the parsed engine error envelope: `code`, `message`,
`details`, and `request_id` when present.

Hybrid batch search mirrors are Rails-side progress records around
`POST /search/batch`. Rails sends each item with the source term `id` and uses
the returned `id` to attach candidate results back to mappings. Missing or
failed item results are recorded in the mirror error payload without aborting
successful items.

`JobsClient#status` must return local mirror status for jobs with no API job ID.
Do not call `/jobs/` for Rails-local mirrors.

Manual searches also create short-lived mirrors. Successful manual searches
store `processed = 1` and candidate count; engine failures store the same error
envelope shape as async jobs so the review page can render a stable failure
state.

## Result Artifacts

`JobsClient#results` first requests `/jobs/:id/results` as JSON. If the engine
returns inline `items`, Rails uses them directly. If the engine returns an
artifact envelope, Rails requests the same endpoint again with
`Accept: application/jsonl` and merges the streamed rows into `items`.

JSONL stream failures must propagate as `EngineClients::BaseClient::Error`.
Persistence jobs should mark the Rails mirror `failed` with `code`, `message`,
`details`, and `request_id` before re-raising. Do not convert a stream failure
into an empty candidate list.

Mapper JSONL rows may contain item-level `error` payloads even when the row does
not include an explicit failed `state`. Rails must persist successful rows and
copy failed rows into `engine_jobs.error.items` so the job page can show partial
failure details and a retry action.
