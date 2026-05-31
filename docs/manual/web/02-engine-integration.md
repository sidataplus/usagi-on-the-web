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
safe stub mode for offline tests
```

## Service Families

Rails uses:

```text
CatalogClient
SearchClient
MapperClient
JobsClient
```

Drug projects use `mapper-api`. Non-drug projects use hybrid search.

## Job Mirrors

Rails stores an `engine_jobs` row for every project-level engine workflow. The
API job ID is a mirror of `usagi-api` state, not a license to inspect
`jobs.sqlite`.
