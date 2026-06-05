# Usagi-on-the-Web v3 Rails Implementation Spec

Status: draft v0.2  
Scope: Rails 8 product workflow layer for Usagi-on-the-Web v3  
Engine dependency: stable `usagi-api` endpoints  
Primary design style: Basecamp/37signals-inspired Rails monolith, server-rendered HTML, custom CSS, Turbo, Stimulus sprinkles

---

## 0. Goal

Build the Rails 8 web application for Usagi-on-the-Web v3.

Rails is the product layer:

```text
Rails
  users
  projects
  imports
  source terms
  mappings
  mapping candidates
  review workflow
  comments
  audit
  exports
  project bundles
  engine job mirrors
  UI
```

`usagi-api` is the engine layer:

```text
usagi-api
  standard-only OMOP catalog
  Tantivy lexical search
  SapBERT CLS + USearch dense search
  hybrid RRF search
  THIRAWAT Drug mapper
  Tachiom retrieval
  external BiMaxSim rerank
  async engine jobs
```

Rails must not read or mutate engine artifacts directly:

```text
catalog.sqlite
Tantivy indexes
USearch vectors
THIRAWAT token embeddings
Tachiom indexes
model weights
jobs.sqlite
```

Rails talks to `usagi-api` through stable HTTP endpoints only.

---

## 1. Product stance

Usagi-on-the-Web v3 is a Rails-first mapping workbench.

The first usable Rails version should feel like a small, durable, focused tool:

```text
Create project
Import source terms
Review mappings
Search candidates
Run auto-suggest when appropriate
Approve / flag / invalidate
Export results
```

No SPA rewrite.  
No dashboard labyrinth.  
No component-industrial complex.  
No direct browser-to-engine calls.  
No model/index artifacts leaking into Rails.

---

## 2. Confirmed decisions

| Area | Decision |
|---|---|
| Rails version | Rails 8.x |
| UI architecture | Server-rendered Rails views with Turbo and Stimulus |
| CSS | Custom CSS, Basecamp/Campfire-inspired organization |
| Component strategy | ERB partials first; helpers and small presenters before heavier abstractions |
| JS strategy | Stimulus for small interactions only |
| Product DB | PostgreSQL for production; SQLite acceptable for dev/test |
| Jobs | Active Job + Solid Queue |
| File storage | Active Storage |
| Cache | Solid Cache for low-risk engine metadata |
| Auth | Rails-owned authentication |
| Authorization | Rails-owned project roles |
| Engine access | Rails service clients only |
| Browser access to API | Prohibited in production |
| API security | Network isolation + signed service requests; optional mTLS later |
| Drug auto-suggest | THIRAWAT/Tachiom/BiMaxSim Drug mapper |
| Non-drug auto-suggest | Hybrid RRF search using Tantivy + SapBERT/USearch |
| Auto-approval | Disabled by default |
| Translation | Deferred optional module |
| Deployment | Docker Compose locally; Kamal later |

---

## 3. Architecture

```text
Browser
  |
  | HTML, forms, Turbo, Stimulus
  v
Rails 8 app
  |
  | signed internal HTTP requests
  v
usagi-api services
  |
  +-- catalog-api
  +-- search-api
  +-- mapper-api
  +-- jobs-api/api-worker
```

Production rule:

```text
Browser -> Rails -> usagi-api
```

Local development may expose engine ports for debugging, but production must not.

---

## 4. Repository layout

Use the existing monorepo direction:

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

  docs/
    api/
    rails/
    security/
    testing/

  fixtures/
    contracts/
    source-terms/
    athena-mini/

  infra/
    docker/
    kamal/
```

Rails app root:

```text
apps/web/
```

---

## 5. Rails app structure

```text
apps/web/app/
  assets/
    stylesheets/
      application.css
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

  controllers/
    application_controller.rb
    sessions_controller.rb
    projects_controller.rb
    import_sessions_controller.rb
    mappings_controller.rb
    mapping_candidates_controller.rb
    manual_searches_controller.rb
    auto_maps_controller.rb
    engine_jobs_controller.rb
    exports_controller.rb
    comments_controller.rb

    admin/
      engine_status_controller.rb
      engine_builds_controller.rb

  jobs/
    import_source_file_job.rb
    start_auto_map_job.rb
    run_hybrid_search_job.rb
    poll_engine_job_job.rb
    persist_mapper_results_job.rb
    persist_hybrid_search_results_job.rb
    create_export_job.rb

  models/
    user.rb
    project.rb
    project_member.rb
    import_session.rb
    source_term.rb
    mapping.rb
    mapping_candidate.rb
    engine_job.rb
    export.rb
    audit_event.rb
    comment.rb

  services/
    engine_clients/
    imports/
    mappings/
    exports/
    projects/

  views/
  javascript/controllers/
```

---

## 6. Design principles

Use the “enough, but not too much” rule.

Every screen should answer:

```text
Where am I?
What object am I working on?
What can I do next?
What changed?
What needs attention?
```

Avoid:

```text
generic dashboards
nested settings mazes
modal stacks
over-configurable tables
chart decoration
SPA loading ceremonies
fake progress percentages
```

Visual style:

```text
large readable text
spacious cards
soft borders
obvious buttons
plain forms
friendly empty states
small color palette
simple status badges
```

---

## 7. Core user flows

### 7.1 Create project

```text
Projects index
  -> New project form
  -> Project created
  -> Project overview
```

Project fields:

```text
name
description
mapping_domain
source_vocabulary
target_domain_ids
target_vocabulary_ids
vocabulary_version
```

Auto-suggest mode is derived:

| Project domain | Default suggestion mode |
|---|---|
| Drug | `drug_mapper` |
| Non-drug single domain | `hybrid_search` |
| Mixed | `hybrid_search` with optional row-level domain filters |

### 7.2 Import source terms

```text
Project
  -> Import
  -> Upload CSV/XLSX
  -> Preview detected columns
  -> Confirm source_code/source_name/source_frequency/domain_hint columns
  -> ImportSourceFileJob
  -> Source terms + mappings created
  -> Review mappings
```

Rails owns this fully. No engine call is required for import.

### 7.3 Manual candidate search

```text
Mapping row
  -> Candidate drawer
  -> Search text defaults to source_name
  -> Rails calls POST /search/concepts
  -> Show hybrid candidates
  -> Reviewer picks candidate
  -> Mapping updated
  -> Audit event written
```

Use hybrid RRF by default:

```json
{
  "q": "source term name",
  "mode": "hybrid_rrf",
  "limit": 20,
  "filters": {
    "domain_id": ["Drug"]
  }
}
```

For non-drug projects, this remains first-class.

### 7.4 Drug auto-suggest

Drug projects use mapper API:

```text
User clicks "Suggest drug mappings"
  -> Rails creates engine_jobs row
  -> StartAutoMapJob
  -> POST /mapper/drugs/batch-job
  -> API returns job_id
  -> Rails polls GET /jobs/:id
  -> Rails fetches GET /jobs/:id/results
  -> Rails persists mapping_candidates
  -> UI updates candidate availability
```

Default behavior:

```text
Persist candidates only.
Do not auto-approve.
Do not overwrite reviewed mappings.
```

### 7.5 Non-drug auto-suggest

Non-drug projects use hybrid search:

```text
User clicks "Suggest candidates"
  -> Rails creates local engine_jobs row with kind = hybrid_search_batch
  -> RunHybridSearchJob chunks source terms
  -> Rails calls POST /search/batch
  -> Rails persists mapping_candidates
  -> UI updates progress via Turbo
```

Suggested chunking:

```text
batch size = 100 source terms
limit per term = 20
mode = hybrid_rrf
lexical_top_k = 100
sapbert_top_k = 100
rrf_k = 60
```

Complexity:

```text
Let P = source terms
Let B = batch size
Let S(q) = search cost per query

Rails orchestration: O(P)
Engine search: O(P * S(q))
Persistence: O(P * K), where K = candidates per source term
```

### 7.6 Mapping review

```text
Project mappings
  -> Filter/search/sort
  -> Open candidate drawer
  -> Compare candidates
  -> Apply candidate
  -> Approve / flag / invalidate
  -> Comment if needed
  -> Audit event written
```

Mapping statuses:

```text
UNCHECKED
APPROVED
FLAGGED
INVALID
```

Equivalence choices:

```text
Equivalent
Narrower
Broader
Related
No match
Unclear
```

### 7.7 Export

```text
Project
  -> Exports
  -> Choose format
  -> CreateExportJob
  -> Export attached with Active Storage
  -> Download
```

MVP formats:

```text
USAGI-compatible CSV
SOURCE_TO_CONCEPT_MAP CSV
review CSV
candidate JSONL
candidate CSV
audit CSV
```

---

## 8. Data model summary

Use string IDs with prefixes.

Core tables:

```text
users
projects
project_members
import_sessions
source_terms
mappings
mapping_candidates
engine_jobs
exports
audit_events
comments
```

See `docs/rails/02_domain-model.md` for migrations, indexes, validations, and lifecycle rules.

---

## 9. Routes summary

Primary routes:

```ruby
root "projects#index"

resource :session, only: %i[new create destroy]

resources :projects do
  resources :import_sessions, only: %i[new create show index]
  resources :mappings, only: %i[index]
  resources :engine_jobs, only: %i[index show]
  resources :exports, only: %i[index create show]
  resource :auto_map, only: %i[create]
end

resources :mappings, only: %i[show update] do
  resources :comments, only: %i[index create]
  resources :mapping_candidates, only: %i[index]
  resource :manual_search, only: %i[create]
  post :apply_candidate, on: :member
  post :approve, on: :member
  post :flag, on: :member
  post :invalidate, on: :member
end

post "/mappings/bulk_update", to: "mappings#bulk_update"

namespace :admin do
  resource :engine_status, only: %i[show]
  resources :engine_builds, only: %i[create]
end
```

See `docs/rails/03_routes-controllers-views.md` for controller and view responsibilities.

---

## 10. Secure Rails-to-usagi-api mechanism

The engine APIs must only accept calls from Rails.

Use defense in depth:

```text
1. Network isolation
2. Firewall/private network
3. Signed service requests
4. Request ID propagation
5. Replay protection
6. Optional mTLS later
```

Signed service requests include:

```http
X-Request-Id: req_...
X-Usagi-Service: rails
X-Usagi-Timestamp: 2026-05-31T12:00:00Z
X-Usagi-Nonce: nonce_...
X-Usagi-Content-SHA256: hex_sha256_body
X-Usagi-Signature: v1=base64_hmac
```

See `docs/security/rails-api-boundary.md` for the complete security contract.

---

## 11. Engine clients

All engine clients must support:

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

HTTP transport rules:

```text
GET and JSONL result fetches may retry transient connection failures
POST requests are not retried by transport
structured logs include request_id, method, path, status, attempt, duration
retry logs include request_id, method, path, attempt, next_attempt, error class
```

Job creation retry safety belongs to API idempotency keys plus Rails
`engine_jobs` mirror reuse. Do not add blind transport-level POST retries.

Expected clients:

```text
EngineClients::BaseClient
EngineClients::CatalogClient
EngineClients::SearchClient
EngineClients::MapperClient
EngineClients::JobsClient
EngineClients::RequestSigner
```

---

## 12. Testing stance

Required test categories:

```text
model tests
service tests
request tests
job tests
system tests
contract fixture tests
signed request security tests
```

Critical workflows:

```text
login
create project
import source terms
manual hybrid search
non-drug hybrid auto-suggest
Drug mapper auto-suggest
apply candidate
approve mapping
export STCM
engine offline state
invalid API signature state
```

See `docs/testing/contract-fixtures.md` for engine fixture contracts.

---

## 13. Development milestones

| Milestone | Outcome |
|---:|---|
| M0 | Rails scaffold, auth, project shell |
| M1 | Product data model |
| M2 | Import workflow |
| M3 | Mapping review workspace |
| M4 | Engine clients + security |
| M5 | Manual hybrid search |
| M6 | Non-drug hybrid auto-suggest |
| M7 | Drug mapper auto-suggest |
| M8 | Exports |
| M9 | Product polish |
| M10 | Deployment hardening |

See `docs/rails/00_development-plan.md` for detailed acceptance criteria.

---

## 14. MVP acceptance checklist

A Rails MVP is acceptable when:

```text
A user can log in.
A user can create a project.
A user can import CSV/XLSX source terms.
Rails creates source_terms and mappings.
A reviewer can filter and review mappings.
A reviewer can manually search hybrid candidates.
A non-drug project can run hybrid auto-suggest.
A Drug project can run THIRAWAT auto-suggest through usagi-api.
Rails mirrors engine job progress.
Rails persists candidates and provenance.
A reviewer can apply a candidate without auto-approving.
A reviewer can approve/flag/invalidate mappings.
Audit events are written.
Exports work for USAGI CSV and SOURCE_TO_CONCEPT_MAP.
Engine offline state does not crash Rails pages.
Browser cannot call usagi-api directly in production.
usagi-api rejects unsigned or invalid Rails service requests.
Contract tests pass.
Live Rails workflow E2E passes against usagi-api before claiming engine readiness.
System tests cover import-review-export.
```

---

## 15. Final implementation stance

Build the Rails app as a focused product workbench:

```text
Rails owns the work.
usagi-api owns the engines.
Turbo moves HTML.
Stimulus adds small behavior.
Custom CSS carries the visual system.
PostgreSQL stores product truth.
Solid Queue runs product jobs.
Active Storage stores user-facing files.
Signed requests protect engine access.
```

Drug gets the specialized THIRAWAT mapper.

Non-drug gets hybrid Tantivy + SapBERT/USearch candidate suggestions.

Humans approve mappings. Machines can suggest. Let us keep at least that tiny hierarchy intact.
