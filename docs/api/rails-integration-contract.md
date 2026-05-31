# Rails integration contract for usagi-api

Status: draft v0.1  
Audience: Rails implementers and API implementers  
Scope: How Rails 8 will consume the stabilized `usagi-api`

## 1. Purpose

Rails is the product workflow layer.

`usagi-api` is the engine layer.

```text
Rails
  projects
  source terms
  mappings
  review UI
  candidates
  imports
  exports
  users
  audit
  job mirrors

usagi-api
  SQLite catalog truth
  Tantivy lexical search
  SapBERT CLS + USearch
  THIRAWAT-SapBERT Drug mapper
  Tachiom retrieval
  BiMaxSim rerank
  async engine jobs
```

The goal is to stabilize the API first so Rails development can be productive later. Rails should not need to know the internals of Tantivy, USearch, Candle, or Tachiom. Rails has its own problems. Let it have fewer.

## 2. Integration rule

Rails talks to API endpoints, not artifact files.

Rails may store API provenance and API outputs.

Rails must not read or mutate:

```text
catalog.sqlite
Tantivy index files
USearch vectors
THIRAWAT token embeddings
Tachiom index
model weights
jobs.sqlite
```

## 3. API endpoints Rails will depend on

### 3.1 Readiness

```text
GET /catalog/status
GET /search/status
GET /mapper/status
GET /jobs/:id
GET /jobs/:id/events
GET /jobs/:id/results
```

### 3.2 Manual search and candidate lookup

```text
POST /search/concepts
POST /mapper/drugs/query
POST /mapper/drugs/explain
```

### 3.3 Bulk auto-map

```text
POST /mapper/drugs/batch-job
GET  /jobs/:id
GET  /jobs/:id/events
GET  /jobs/:id/results
```

### 3.4 Admin/index operations, if exposed in Rails

```text
POST /catalog/build-job
POST /search/tantivy/build-job
POST /search/sapbert/build-job
POST /mapper/thirawat/build-embeddings-job
POST /mapper/tachiom/build-index-job
```

## 4. Rails-owned data model

Rails owns product state.

Suggested tables:

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

Minimum API integration tables:

```text
projects
source_terms
mappings
mapping_candidates
engine_jobs
exports
audit_events
```

## 5. `engine_jobs` mirror table

Rails mirrors API jobs so the UI can render history, progress, and project-level state without directly reading `jobs.sqlite`.

```ruby
create_table :engine_jobs, id: :string do |t|
  t.string  :project_id
  t.string  :api_job_id, null: false
  t.string  :kind, null: false
  t.string  :state, null: false
  t.string  :stage
  t.integer :processed, null: false, default: 0
  t.integer :total, null: false, default: 0
  t.integer :failed, null: false, default: 0
  t.string  :mode
  t.string  :status_url
  t.string  :result_url
  t.json    :input
  t.json    :result
  t.json    :error
  t.datetime :started_at
  t.datetime :finished_at
  t.timestamps
end

add_index :engine_jobs, :project_id
add_index :engine_jobs, :api_job_id, unique: true
add_index :engine_jobs, [:project_id, :kind]
```

## 6. Mapping candidate persistence

Rails persists candidate outputs returned by `mapper-api`.

```ruby
create_table :mapping_candidates, id: :string do |t|
  t.string  :project_id, null: false
  t.string  :mapping_id, null: false

  t.integer :rank, null: false
  t.integer :concept_id, null: false
  t.text    :concept_name, null: false
  t.string  :domain_id
  t.string  :vocabulary_id
  t.string  :concept_class_id
  t.string  :standard_concept
  t.string  :concept_code

  t.float   :tantivy_score
  t.float   :sapbert_score
  t.float   :rrf_score
  t.float   :tachiom_maxsim_score
  t.float   :bimaxsim_score
  t.float   :tie_breaker_score
  t.float   :final_score

  t.string  :method
  t.json    :features
  t.json    :provenance
  t.timestamps
end

add_index :mapping_candidates, :project_id
add_index :mapping_candidates, :mapping_id
add_index :mapping_candidates, [:mapping_id, :rank]
```

Rails stores candidate provenance because the reviewer needs traceability.

Example provenance:

```json
{
  "catalog_artifact_id": "athena-20250827-standard-v1",
  "thirawat_model_id": "sidataplus/THIRAWAT-SapBERT",
  "tachiom_artifact_id": "athena-20250827-thirawat-drug-tachiom-v1",
  "api_version": "0.1.0"
}
```

## 7. Rails service clients

Rails should centralize API calls.

```text
app/services/engine_clients/
  base_client.rb
  catalog_client.rb
  search_client.rb
  mapper_client.rb
  jobs_client.rb
```

### 7.1 Base client requirements

All clients must support:

```text
base URL from ENV
request ID propagation
JSON serialization
timeouts
error envelope parsing
structured logs
safe retries for GET
idempotency keys for job creation
```

### 7.2 Environment variables

```text
CATALOG_API_URL=http://catalog-api:8788
SEARCH_API_URL=http://search-api:8789
MAPPER_API_URL=http://mapper-api:8790
ENGINE_API_TIMEOUT_SECONDS=30
ENGINE_API_JOB_POLL_INTERVAL_SECONDS=2
```

### 7.3 Example mapper client

```ruby
class EngineClients::MapperClient
  def initialize(base_url: ENV.fetch("MAPPER_API_URL"))
    @base_url = base_url
  end

  def start_drug_batch_job(project:, items:, mode: "thirawat_tachiom")
    post_json("/mapper/drugs/batch-job", {
      idempotency_key: idempotency_key(project, items, mode),
      mode: mode,
      candidate_top_k: 200,
      rerank_top_n: 100,
      limit: 20,
      items: items
    })
  end

  def query_drug(source_name:, source_code: nil)
    post_json("/mapper/drugs/query", {
      source_name: source_name,
      source_code: source_code,
      mode: "thirawat_tachiom",
      candidate_top_k: 200,
      rerank_top_n: 100,
      limit: 20,
      post_rank: {
        mode: "tiebreak",
        epsilon: 0.01,
        top_n: 100
      }
    })
  end
end
```

## 8. Rails workflow integration

## 8.1 Import workflow

Rails owns import.

```text
User uploads CSV/XLSX
  -> Rails stores source file
  -> Rails parses/imports source terms
  -> Rails creates mappings
  -> User may start auto-map
```

API calls:

```text
none required initially
```

Optional later:

```text
POST /translate/batch-job
```

Translation is deferred and optional.

## 8.2 Manual search workflow

```text
Reviewer opens mapping row
  -> Rails calls POST /search/concepts
  -> Rails displays candidates
  -> Reviewer selects candidate
  -> Rails updates mapping
  -> Rails writes audit event
```

Request:

```json
{
  "q": "tramadol 50 mg capsule",
  "mode": "hybrid_rrf",
  "limit": 20,
  "filters": {
    "domain_id": ["Drug"]
  }
}
```

Rails may persist manually searched candidates if useful, but mapping approval remains Rails state.

## 8.3 Single-row mapper workflow

```text
Reviewer requests mapper suggestions for one row
  -> Rails calls POST /mapper/drugs/query
  -> Rails displays candidates
  -> Reviewer selects/approves
```

Rails should only call `/mapper/drugs/query` for Drug-domain rows or rows that the project marks as Drug. API v0.1 THIRAWAT supports Drug only.

## 8.4 Bulk auto-map workflow

```text
User clicks Auto-map
  -> Rails creates engine_jobs row
  -> Rails enqueues StartAutoMapJob
  -> StartAutoMapJob calls POST /mapper/drugs/batch-job
  -> API returns api_job_id
  -> Rails stores api_job_id
  -> Rails enqueues PollEngineJobJob
  -> PollEngineJobJob polls API
  -> Rails broadcasts progress
  -> When succeeded, Rails fetches results
  -> Rails writes mapping_candidates
  -> Rails optionally sets provisional target mappings
```

### Rails job sketch

```ruby
class StartAutoMapJob < ApplicationJob
  queue_as :engine

  def perform(project_id)
    project = Project.find(project_id)

    engine_job = project.engine_jobs.create!(
      kind: "mapper_drugs_batch",
      state: "starting",
      mode: "thirawat_tachiom"
    )

    payload_items = project.source_terms.map do |term|
      {
        id: term.id,
        source_code: term.source_code,
        source_name: term.source_name,
        source_frequency: term.source_frequency
      }
    end

    response = EngineClients::MapperClient.new.start_drug_batch_job(
      project: project,
      items: payload_items,
      mode: "thirawat_tachiom"
    )

    engine_job.update!(
      api_job_id: response.fetch("job_id"),
      state: response.fetch("state"),
      status_url: response.fetch("status_url")
    )

    PollEngineJobJob.set(wait: 2.seconds).perform_later(engine_job.id)
  end
end
```

## 8.5 Polling workflow

```ruby
class PollEngineJobJob < ApplicationJob
  queue_as :engine_polling

  def perform(engine_job_id)
    engine_job = EngineJob.find(engine_job_id)

    status = EngineClients::JobsClient.new.status(engine_job.api_job_id)

    engine_job.update!(
      state: status.fetch("state"),
      stage: status["stage"],
      processed: status.fetch("processed", 0),
      total: status.fetch("total", 0),
      failed: status.fetch("failed", 0),
      started_at: status["started_at"],
      finished_at: status["finished_at"]
    )

    case engine_job.state
    when "queued", "running"
      PollEngineJobJob.set(wait: 2.seconds).perform_later(engine_job.id)
    when "succeeded", "succeeded_with_errors"
      PersistMapperResultsJob.perform_later(engine_job.id)
    when "failed", "cancelled"
      # Broadcast final failure state.
    end
  end
end
```

## 8.6 Persist mapper results workflow

```text
Rails calls GET /jobs/:id/results
  -> receives artifact reference or inline result
  -> streams/parses result JSONL
  -> writes mapping_candidates rows
  -> optionally writes provisional mapping target
  -> writes audit events
```

Rails should preserve all candidate provenance.

## 9. Rails 8 features expected

### 9.1 Active Record

Rails stores:

```text
project state
source terms
mapping decisions
candidate rows
audit events
engine job mirrors
export metadata
```

### 9.2 Active Job and Solid Queue

Rails uses Solid Queue for product workflow jobs:

```text
ImportSourceFileJob
StartAutoMapJob
PollEngineJobJob
PersistMapperResultsJob
CreateExportJob
```

The API has its own job system. Rails does not directly use `jobs.sqlite`.

### 9.3 Turbo Streams

Rails can broadcast:

```text
engine job progress
mapping candidate availability
export readiness
mapping row updates
```

Example:

```ruby
class EngineJob < ApplicationRecord
  after_update_commit -> {
    broadcast_replace_to(
      [project, :engine_jobs],
      target: "engine_job_#{id}",
      partial: "engine_jobs/card",
      locals: { engine_job: self }
    )
  }
end
```

### 9.4 Active Storage

Rails uses Active Storage for:

```text
uploaded source files
export artifacts
project bundles
```

The API stores engine artifacts, not user-facing project artifacts.

### 9.5 Solid Cache

Rails may cache low-risk engine metadata:

```text
catalog status
search status
mapper status
domain/vocabulary/concept-class lists
```

Do not cache mapping decisions as truth.

## 10. API status dashboard in Rails

Rails should include an admin/runtime page that calls:

```text
GET /catalog/status
GET /search/status
GET /mapper/status
```

Display:

```text
catalog artifact ID
vocabulary version
standard concept count
Tantivy index status
SapBERT index status
THIRAWAT model status
Tachiom index status
Drug mapper readiness
```

## 11. Rails permissions

Rails handles users and authorization. `usagi-api` is internal and does not implement project/user permissions in API v0.1.

Deployment rule:

```text
Browser -> Rails
Rails -> usagi-api
```

The browser should not call `mapper-api` directly in production.

## 12. Rails-side mapping status

Rails owns mapping decisions.

Suggested status values:

```text
UNCHECKED
APPROVED
FLAGGED
INVALID
```

Rails owns:

```text
target_concept_id
mapping_status
equivalence
reviewed_by
reviewed_at
row_version
```

The API only suggests candidates.

## 13. Candidate application logic

When Rails auto-applies a top candidate, this should be explicit and configurable.

Suggested rules:

```text
if top candidate final_score >= threshold
and no warnings
and candidate rank = 1
then set provisional target
else leave unchecked with candidates
```

Initial recommendation:

```text
do not auto-approve
populate candidates only
```

Let humans approve until quality is measured. Humanity has few uses, but reviewing drug mappings is still one of them.

## 14. Failure handling

### 14.1 API unavailable

Rails should show:

```text
engine offline
retry later
diagnostics link
```

Rails should not crash project pages because search is unavailable.

### 14.2 API job failed

Rails updates `engine_jobs.state = failed` and stores `error`.

Rails UI shows:

```text
failure stage
error code
message
retry button if safe
```

### 14.3 Partial mapper failure

If API returns `succeeded_with_errors`, Rails should:

```text
persist successful candidate rows
record failed source terms
show failed count
allow retry failed items
```

## 15. Stable contract before Rails productivity

Rails front-end work should begin after these API capabilities are stable:

```text
catalog status and concept lookup
Tantivy lexical search
SapBERT hybrid search
THIRAWAT Drug mapper query
THIRAWAT Drug mapper batch-job
jobs polling and results
error envelope
artifact provenance
```

Minimum API stable checkpoint:

```text
POST /mapper/drugs/batch-job
GET  /jobs/:id
GET  /jobs/:id/results
```

must be reliable with fixture data.

## 16. Contract tests

Rails integration test suite should run against fixture API or mocked API responses.

Required contract fixtures:

```text
fixtures/contracts/
  catalog_status.ready.json
  search_concepts.hybrid_rrf.request.json
  search_concepts.hybrid_rrf.response.json
  mapper_drugs_query.request.json
  mapper_drugs_query.response.json
  mapper_drugs_batch_job.request.json
  mapper_drugs_batch_job.response.json
  jobs_status.running.json
  jobs_results.mapper_success.json
  error.index_not_ready.json
```

Rails should not depend on undocumented fields.

## 17. Product workflow boundary

Rails product endpoints may look like:

```text
GET  /projects
POST /projects
GET  /projects/:id
POST /projects/:id/import_sessions
GET  /projects/:id/mappings
PATCH /mappings/:id
POST /projects/:id/auto_maps
GET  /projects/:id/exports
POST /projects/:id/exports
```

These are not API engine endpoints. They are Rails user-facing endpoints.

## 18. Deferred translation integration

Translation is optional and later.

When implemented:

```text
Rails source terms may store:
  source_name_translated
  translation_model_id
  translation_quantization
  translation_warnings
  translation_updated_at
```

Mapper may later accept:

```text
source_name
source_name_translated
mode = raw_plus_translated
```

But API v0.1 Rails integration must not require translation.

## 19. Summary

Rails depends on stable engine endpoints and stable DTOs.

Rails owns:

```text
user workflow
review decisions
project persistence
exports
audit
UI
```

API owns:

```text
catalog
search
embedding
THIRAWAT Drug mapper
Tachiom retrieval
engine jobs
```

The integration contract is simple on purpose. Simple contracts are how Rails later becomes productive instead of joining the ancient human ceremony of debugging half-stable internal APIs while pretending it is progress.
