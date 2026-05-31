# AGENTS.md

Canonical instructions for coding agents working in **Usagi-on-the-Web v3**.

The project is now moving from API-spine stabilization into the Rails product
stage. Build Rails as the workflow layer on top of stable `usagi-api` endpoints.
Do not move engine internals into Rails, and do not let the browser bypass Rails
in production.

---

## 1. Project Identity

Usagi-on-the-Web v3 is an OMOP concept mapping workbench.

```text
Browser
  -> Rails 8 app
    -> usagi-api internal services
```

Rails owns product workflow:

```text
users
projects
project membership
imports
source terms
mappings
mapping candidates
comments
audit events
exports
project bundles
engine job mirrors
UI
```

`usagi-api` owns engine internals:

```text
catalog.sqlite
Tantivy lexical indexes
SapBERT CLS + USearch indexes
THIRAWAT-SapBERT model artifacts
Tachiom indexes
BiMaxSim reranking
SQLite-backed API job queue
engine job results
```

Rails stores engine outputs and provenance needed for product review. Rails must
not read, write, rebuild, or infer from engine artifact files directly.

---

## 2. Source Documents

Before broad changes, read the relevant docs:

```text
docs/rails/implementation-spec.md
docs/rails/00_development-plan.md
docs/rails/01_product-shape.md
docs/rails/02_domain-model.md
docs/rails/03_routes-controllers-views.md

docs/security/rails-api-boundary.md
docs/testing/contract-fixtures.md

docs/api/implementation-spec.md
docs/api/endpoints.md
docs/api/artifacts.md
docs/api/jobs.md
docs/api/rails-integration-contract.md
```

Use the Rails docs for product behavior, UI shape, domain model, routes, and
workflow. Use the API docs for service boundaries, DTOs, artifact layout, job
behavior, and Rails integration expectations.

---

## 3. Non-Negotiable Architecture Rules

1. The browser must never call `catalog-api`, `search-api`, `mapper-api`, or
   `jobs-api` directly in production.
2. Rails calls `usagi-api` only through service clients in
   `apps/web/app/services/engine_clients/`.
3. Rails-to-engine requests must be signed outside test mode.
4. Rails product state lives in the Rails database.
5. Engine internals are accessed only through documented API endpoints.
6. Rails must not read or mutate engine artifact files.
7. Mapping decisions remain human-owned unless a future spec explicitly changes
   that rule.
8. Auto-suggest may create candidates. It must not silently approve mappings.
9. Drug projects use the THIRAWAT Drug mapper.
10. Non-drug projects use hybrid search with Tantivy + SapBERT/USearch.
11. Server-rendered Rails views are the default.
12. Stimulus is for small behavior, not client-side application state.
13. Custom CSS is the default. Do not add Tailwind, Bootstrap, React, Svelte, or
    a component library unless the human changes the spec.

---

## 4. Current Architectural Decisions

These are settled unless the human explicitly changes them.

| Area | Decision |
|---|---|
| Repository | Single monorepo: `usagi-on-the-web` |
| Current stage | Rails product workflow on top of stable `usagi-api` |
| Rails app root | `apps/web/` |
| Rails version | Rails 8.x |
| UI architecture | Server-rendered Rails views with Turbo and Stimulus |
| CSS | Custom CSS, organized by role |
| Product DB | PostgreSQL for production; SQLite acceptable for dev/test |
| Rails jobs | Active Job + Solid Queue |
| File storage | Active Storage |
| Cache | Solid Cache for low-risk engine metadata |
| Auth | Rails-owned authentication |
| Authorization | Rails-owned project roles |
| Engine access | Rails service clients only |
| API security | Private network + signed service requests; optional mTLS later |
| Local deployment | Docker Compose |
| Server deployment later | Kamal on DigitalOcean |
| Runtime catalog truth | SQLite |
| DuckDB | Dev fixture extraction only; never runtime |
| Mapper model | `sidataplus/THIRAWAT-SapBERT` |
| Mapper domain | Drug only for THIRAWAT v0.1 |
| Translation | Deferred optional module |

---

## 5. Repository Layout

Expected project shape:

```text
usagi-on-the-web/
  apps/
    web/
      app/
      config/
      db/
      test/
      public/
      bin/
      Gemfile
      config.ru

  services/
    catalog-api/
    search-api/
    mapper-api/
    api-worker/

  crates/
    usagi-contracts/
    usagi-common/
    usagi-jobs/
    usagi-artifacts/
    usagi-catalog/
    usagi-search/
    usagi-embed/
    usagi-thirawat/
    usagi-tachiom/

  fixtures/
    athena-mini/
    source-terms/
    dev-drugs-50/
    thirawat-golden/
    search-golden/

  infra/
    docker/
    kamal/

  docs/
    api/
    rails/
    security/
    testing/
```

Rails application code belongs under `apps/web/`. Shared engine logic stays in
Rust crates and services.

---

## 6. Rails Implementation Style

Prefer:

```text
plain Rails models
plain controllers
service objects for engine integration and import/export workflows
ERB partials
Turbo Frames and Turbo Streams
small Stimulus controllers
custom CSS modules
PostgreSQL indexes that match actual queries
small tests near the behavior they verify
contract fixtures for engine responses
```

Avoid:

```text
large inheritance trees
callback jungles
JSON APIs for internal Rails page flow
client-side state stores
premature ViewComponent trees
clever metaprogramming
callbacks that make writes unpredictable
SQL hidden inside views
direct HTTP calls outside engine clients
```

If a thing can be a normal method, make it a normal method.

---

## 7. Rails Directory Conventions

Use this application structure:

```text
apps/web/app/
  controllers/
  models/
  jobs/
  services/
    engine_clients/
    imports/
    mappings/
    exports/
    projects/
  views/
  assets/stylesheets/
  javascript/controllers/
```

Docs live in:

```text
docs/rails/
docs/security/
docs/testing/
docs/api/
```

---

## 8. Naming Rules

Use string IDs with prefixes:

| Model | Prefix |
|---|---|
| User | `usr_` |
| Project | `proj_` |
| ProjectMember | `pmem_` |
| ImportSession | `imp_` |
| SourceTerm | `src_` |
| Mapping | `map_` |
| MappingCandidate | `cand_` |
| EngineJob | `ejob_` |
| Export | `exp_` |
| AuditEvent | `aud_` |
| Comment | `com_` |

Use a small `Ids.generate(prefix)` helper. Do not scatter ad hoc ID generation
through models, controllers, or jobs.

---

## 9. Rails Model Rules

Use Rails validations for local product rules. Use database constraints and
indexes for invariants.

Required invariants:

```text
project_members: unique(project_id, user_id)
source_terms: unique(project_id, source_code)
mappings: unique(source_term_id)
mapping_candidates: unique(mapping_id, concept_id, method)
engine_jobs: unique(api_job_id) when api_job_id is not null
```

Use Rails optimistic locking on `mappings.lock_version`.

Do not bypass model-level audit helpers for mapping decision writes unless
writing a migration or backfill.

---

## 10. Controller Rules

Controllers should:

```text
authenticate user
authorize project/action
load objects
call service objects for non-trivial work
respond with HTML/Turbo Stream
```

Controllers should not:

```text
parse files directly
call engine APIs directly outside service clients
contain candidate persistence logic
perform bulk mapping logic inline
build export files inline
```

---

## 11. Engine Client Rules

All engine clients must inherit from or delegate to `EngineClients::BaseClient`.

Required behavior:

```text
base URL from ENV
request ID propagation
signed request headers
JSON serialization
error envelope parsing
timeouts
safe GET retries
structured logging
idempotency keys for job creation
```

Do not call `Net::HTTP`, Faraday, HTTParty, or similar libraries from random
application code. Use the engine clients.

---

## 12. UI Rules

Use Basecamp/37signals-style product principles:

```text
few durable places
clear objects
spacious cards
plain language
server-rendered HTML
Turbo for page flow
Stimulus sprinkles
obvious buttons
friendly empty states
```

Do not create:

```text
generic dashboard grids
component showrooms
icon-only toolbars without labels
low-contrast enterprise gray UI
configurable everything
fake progress bars for unknown work
```

Use copy that helps the reviewer understand what to do next.

---

## 13. CSS Rules

Use custom CSS files organized by role:

```text
reset.css
colors.css
base.css
layout.css
typography.css
buttons.css
inputs.css
cards.css
tables.css
nav.css
projects.css
mappings.css
candidates.css
jobs.css
exports.css
utilities.css
```

Use semantic classes:

```css
.project-card {}
.mapping-table {}
.status-badge {}
.candidate-card {}
.job-card {}
```

Avoid atomic class soup unless it is a tiny utility.

---

## 14. Turbo And Stimulus Rules

Use Turbo Frames for:

```text
candidate drawer
manual search results
job cards
import preview
export list
admin status cards
```

Use Turbo Streams for:

```text
job progress
mapping row replacement
candidate list update
export readiness
flash messages
```

Use Stimulus only for:

```text
row selection
keyboard shortcuts
drawer behavior
file upload preview
auto-submit filters
copy-to-clipboard
```

No client-side source of truth for mapping decisions.

---

## 15. API Boundary Rules To Preserve

The API layer remains the engine room. Rails consumes stable endpoints and owns
workflow.

Do not implement or reintroduce these unless explicitly asked:

```text
Desktop app
Serverless edge / Cloudflare Worker / D1 / Durable Objects
Separate usagi-api repository
DuckDB as runtime catalog truth
LLM candidate reordering
TranslateGemma / translate-api during initial Rails development
Non-drug THIRAWAT mapping
Python CLI wrapper as production mapper
Browser-direct calls to mapper-api
```

Runtime catalog building must produce SQLite and must load only:

```sql
standard_concept = 'S'
AND coalesce(invalid_reason, '') = ''
```

DuckDB may be used only as an external development source for fixture extraction
or bootstrap scripts. Do not make DuckDB a runtime dependency of
`catalog-api`, `search-api`, `mapper-api`, or Rails.

---

## 16. Engine Contract Summary

Rails should consume these service families through engine clients:

```text
catalog-api
  standard concept lookup
  batch lookup
  ancestors/descendants
  domains/vocabularies/classes

search-api
  lexical_tantivy
  sapbert_cls
  hybrid_rrf
  concept search, batch search, explain

mapper-api
  THIRAWAT Drug mapper
  Tachiom retrieval
  exact BiMaxSim rerank
  deterministic drug tie-breaker
  drug query, batch, batch job, explain

jobs-api / api-worker
  catalog_build
  tantivy_build
  sapbert_build
  thirawat_doc_embed
  tachiom_build
  mapper_drugs_batch
```

All API errors use the standard envelope:

```json
{
  "error": {
    "code": "INDEX_NOT_READY",
    "message": "SapBERT index is not built",
    "details": {
      "required_artifact": "sapbert_cls.usearch"
    },
    "request_id": "req_abc"
  }
}
```

Rails clients must parse this envelope and preserve `request_id` in logs and
user-safe error states.

---

## 17. Testing Requirements

Every behavior change needs tests before it is considered done.

Rails feature work should include the relevant categories:

```text
model tests
service tests
request tests
job tests
system tests for critical workflows
contract fixture tests for engine responses
security tests for signed API requests
```

Do not merge Rails code that only works against a live engine. Use contract
fixtures and fake clients for most tests. Keep live engine checks for smoke tests
and integration validation.

Engine work should continue to satisfy the relevant Rust gates:

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Rails work should use the commands provided by `apps/web` once the app exists.
Prefer `bin/rails test` and targeted system/request tests over broad,
unexplained runs.

---

## 18. Generated And Large Files

Do not commit:

```text
*.duckdb
*.sqlite
*.sqlite3
*.usearch
*.safetensors
*.gguf
*.npy
*.npz
*.arrow
*.parquet
/data/
/target/
```

Exceptions require explicit human approval and should usually be tiny fixtures.

---

## 19. When To Ask The Human

Ask before changing any of these:

```text
service boundaries
repository split
runtime catalog storage
DuckDB runtime use
THIRAWAT non-drug support
LLM support
translation module implementation timing
Rails data ownership
browser-to-engine access rules
artifact format changes
model runtime fallback away from Candle
component framework or CSS framework adoption
```

If implementation discovers that Candle cannot load or reproduce
THIRAWAT-SapBERT correctly, stop and report the exact failure with reproduction
steps. Do not silently swap in Python, ONNX, or another runtime.

---

## 20. Commit Discipline

Each implementation slice should include:

```text
migration/model changes
application code
tests
minimal UI if user-facing
doc update if behavior changes
```

Keep commits small, boring, and reviewable.

---

## 21. Done Definition

A task is done when:

```text
implementation matches the relevant doc
tests pass
failure states are handled
no browser-to-engine shortcut was introduced
no engine artifacts are read from Rails
UI has an empty/error state where applicable
logs include request IDs for engine calls
uncertainty or skipped verification is stated
```
