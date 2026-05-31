# Rails Development Plan

Status: draft v0.1  
Scope: implementation sequence for `apps/web`, the Rails 8 product layer

---

## 0. Goal

This plan turns the Rails implementation spec into development slices that can be built, reviewed, and tested.

The plan deliberately avoids “build the whole app” milestones, because that phrase is how people lose months and gain opinions about CSS they never wanted.

---

## 1. Development principles

1. Ship thin vertical slices.
2. Keep Rails workflow state independent from engine availability.
3. Mock `usagi-api` in Rails tests using contract fixtures.
4. Add the secure Rails-to-API boundary before any production deployment.
5. Build server-rendered pages first; add Turbo/Stimulus only where it improves the flow.
6. Do not implement translation, project bundles, or non-drug specialized mappers in MVP.
7. Make each milestone pass tests before moving on.
8. Optimize for desktop reviewers only; responsive/mobile UI is not a product requirement.

---

## 2. Milestone overview

| Milestone | Name | Main outcome |
|---:|---|---|
| M0 | Rails scaffold | Rails app boots with auth, CSS shell, project index |
| M1 | Product data model | Core models, migrations, associations, validations |
| M2 | Import workflow | CSV/XLSX import creates source terms and mappings |
| M3 | Review workspace | Mapping table, filters, drawer shell, status workflow |
| M4 | Engine clients + security | Signed Rails-to-API clients and engine status admin page |
| M5 | Manual hybrid search | Reviewer can search and apply candidates |
| M6 | Non-drug hybrid auto-suggest | Background batch hybrid search persists candidates |
| M7 | Drug mapper auto-suggest | API mapper jobs are started, polled, persisted |
| M8 | Exports | CSV/JSONL export jobs and downloads |
| M9 | Product polish | Empty states, shortcuts, failure states, audit polish |
| M10 | Deployment hardening | Docker/Kamal shape, private engine network, boot checks |

---

## 3. M0: Rails scaffold

### Goal

Create the Rails app in `apps/web` and make it boot reliably.

### Deliverables

```text
apps/web/
  app/
  config/
  db/
  test/
  Gemfile
```

Initial capabilities:

```text
Rails 8 app
PostgreSQL configuration
Solid Queue configuration
Active Storage configuration
custom CSS skeleton
simple Rails-owned authentication
project index placeholder
application layout
```

### Tasks

| Task | Output |
|---|---|
| Generate Rails app | `apps/web` |
| Configure PostgreSQL | `config/database.yml` |
| Configure Solid Queue | queue tables and worker config |
| Configure Active Storage | local disk for dev/test |
| Add authentication skeleton | `User`, sessions controller |
| Add CSS file structure | app stylesheets imported by `application.css` |
| Add base layout | header, main content, flash area |
| Add root route | projects index |
| Add test helpers | login helpers, ID helpers |

### Acceptance criteria

```text
bin/rails db:prepare succeeds
bin/rails test succeeds
user can log in locally
root path renders
CSS loads
no engine dependency required to boot
```

---

## 4. M1: Product data model

### Goal

Implement the Rails-owned product state.

### Deliverables

Models:

```text
User
Project
ProjectMember
ImportSession
SourceTerm
Mapping
MappingCandidate
EngineJob
Export
AuditEvent
Comment
```

### Tasks

| Task | Output |
|---|---|
| Add ID helper | string IDs with prefixes |
| Add migrations | all core tables |
| Add associations | model relationships |
| Add validations | required fields, enum inclusion |
| Add indexes | query and uniqueness indexes |
| Add optimistic locking | `mappings.lock_version` |
| Add policy layer | project roles and permissions |
| Add counters/scopes | project review counts |

### Acceptance criteria

```text
all models validate expected states
unique source_code per project enforced
one mapping per source_term enforced
candidate uniqueness enforced
project role permissions tested
project overview can compute counts
```

---

## 5. M2: Import workflow

### Goal

Let users import source terms from CSV/TSV/XLSX and create initial mappings.

### Deliverables

```text
ImportSessionsController
ImportSourceFileJob
Imports::CsvParser
Imports::XlsxParser
Imports::ColumnDetector
Imports::SourceTermImporter
import preview views
import result views
```

### Flow

```text
Upload file
  -> detect format and headers
  -> preview rows
  -> confirm column mapping
  -> import confirmed rows
  -> create source_terms
  -> create mappings
  -> write audit event
```

Current Rails scaffold note: preview persists the uploaded/pasted import payload,
detected columns, and suggested mapping. Confirming the preview imports rows
synchronously; a background import job remains the expected hardening path for
large files.

### Tasks

| Task | Output |
|---|---|
| File upload form | Active Storage source file attachment |
| CSV/TSV parser | encoding and delimiter handling |
| XLSX parser | row extraction with `roo` |
| Column detector | suggested source_code/source_name/frequency/domain_hint |
| Preview page | first N rows and warnings |
| Import job | batch insert source terms and mappings |
| Error reporting | row-level validation errors |
| Audit | `source_terms_imported` event |

### Acceptance criteria

```text
CSV fixture imports successfully
XLSX fixture imports successfully
source terms created
mappings created with UNCHECKED status
duplicate source codes rejected or reported clearly
invalid rows do not crash import
import progress/final state visible
```

### Complexity

Let:

```text
N = rows
W = columns
```

Expected complexity:

```text
parse: O(N * W)
validate: O(N)
bulk insert: O(N)
initial mapping creation: O(N)
```

---

## 6. M3: Review workspace

### Goal

Provide the central review UI: filters, table, candidate drawer shell, mapping status updates, comments, and audit.

### Deliverables

```text
MappingsController
MappingCandidatesController
CommentsController
Mappings::StatusUpdater
Mappings::BulkStatusUpdater
Mappings::AuditWriter
mapping table partials
candidate drawer Turbo Frame
row selection Stimulus controller
keyboard shortcuts Stimulus controller
```

### Tasks

| Task | Output |
|---|---|
| Mapping index | server-side filters/sorts/pagination |
| Mapping row partial | Turbo-replaceable row |
| Candidate drawer shell | source term, current mapping, tabs |
| Status actions | approve, flag, invalidate |
| Bulk actions | selected row update |
| Comments | add/list comments |
| Audit history | row-level event list |
| Keyboard shortcuts | j/k/Enter/a/f/x/Esc |

### Acceptance criteria

```text
reviewer can filter mappings
reviewer can approve/flag/invalidate one row
reviewer can bulk update selected rows
mapping updates increment lock_version
audit events are written
comments persist and render
candidate drawer opens via Turbo
```

---

## 7. M4: Engine clients + security

### Goal

Build the Rails-to-`usagi-api` integration layer with signed requests and a diagnostic admin page.

### Deliverables

```text
EngineClients::BaseClient
EngineClients::RequestSigner
EngineClients::CatalogClient
EngineClients::SearchClient
EngineClients::MapperClient
EngineClients::JobsClient
Admin::EngineStatusController
engine status views
security tests
```

### Tasks

| Task | Output |
|---|---|
| Base HTTP client | JSON, timeouts, error parsing |
| Request signer | HMAC headers |
| Request ID propagation | `X-Request-Id` |
| Error envelope parser | engine error object |
| Search client | `/search/status`, `/search/concepts`, `/search/batch` |
| Mapper client | `/mapper/status`, `/mapper/drugs/query`, `/mapper/drugs/batch-job` |
| Jobs client | `/jobs/:id`, `/jobs/:id/events`, `/jobs/:id/results` |
| Admin page | catalog/search/mapper status cards |

### Acceptance criteria

```text
mocked engine calls pass
signed headers match expected canonical string
engine offline renders useful admin state
engine errors preserve code/message/request_id
no controller calls engine directly
```

---

## 8. M5: Manual hybrid search

### Goal

Let reviewers search OMOP candidates from a mapping row.

### Deliverables

```text
ManualSearchesController
Mappings::CandidatePersister
candidate list partials
concept result partials
```

### Flow

```text
Reviewer opens drawer
  -> submits search
  -> Rails calls /search/concepts
  -> Rails persists candidates
  -> Turbo replaces candidate list
  -> reviewer applies candidate
```

### Acceptance criteria

```text
manual search calls SearchClient
hybrid search response persists candidates
candidate provenance persists
candidate can be applied
mapping remains UNCHECKED until approved
manual search failures show friendly error
```

---

## 9. M6: Non-drug hybrid auto-suggest

### Goal

Enable non-drug and mixed projects to get candidate suggestions using hybrid search.

### Deliverables

```text
RunHybridSearchJob
PersistHybridSearchResultsJob or inline persistence service
engine job card
Turbo progress updates
```

### Flow

```text
User clicks Suggest candidates
  -> AutoMapsController chooses hybrid_search
  -> EngineJob mirror row created
  -> RunHybridSearchJob chunks source terms
  -> POST /search/batch
  -> candidates persisted
  -> job progress broadcast
```

### Acceptance criteria

```text
Condition project runs hybrid search suggestions
Measurement project runs hybrid search suggestions
Mixed project uses row/domain filters when present
100k candidate insert path tested
partial batch failure records failed count
engine offline does not break review page
```

---

## 10. M7: Drug mapper auto-suggest

### Goal

Enable Drug projects to use the specialized THIRAWAT mapper through async API jobs.

### Deliverables

```text
StartAutoMapJob
PollEngineJobJob
PersistMapperResultsJob
engine job mirror UI
partial failure UI
retry affordance
```

### Flow

```text
User clicks Suggest drug mappings
  -> StartAutoMapJob creates engine job mirror
  -> POST /mapper/drugs/batch-job
  -> API job ID stored
  -> PollEngineJobJob polls /jobs/:id
  -> on success fetch /jobs/:id/results
  -> persist mapping_candidates
```

### Acceptance criteria

```text
drug project starts mapper batch job
api_job_id stored
polling updates processed/total/failed/stage
succeeded results persist candidates
succeeded_with_errors persists successes and failed count
failed job renders retry path
reviewed mappings are not overwritten
```

---

## 11. M8: Exports

### Goal

Generate user-facing export files.

### Deliverables

```text
ExportsController
CreateExportJob
Exports::UsagiCsvExporter
Exports::SourceToConceptMapExporter
Exports::ReviewCsvExporter
Exports::CandidateJsonlExporter
Exports::AuditCsvExporter
export history views
```

### MVP formats

```text
USAGI-compatible CSV
SOURCE_TO_CONCEPT_MAP CSV
review CSV
candidate JSONL
candidate CSV
audit CSV
```

### Acceptance criteria

```text
export job creates Active Storage file
USAGI CSV downloads
STCM CSV includes approved rows by default with OMOP source-to-concept columns
candidate JSONL downloads and includes provenance
failed export shows error state
```

---

## 12. M9: Product polish

### Goal

Make the app usable by real reviewers.

### Deliverables

```text
friendly empty states
keyboard shortcut help
better error states
job status cards
admin diagnostics
review count summaries
desktop-first review layout
audit polishing
```

### Acceptance criteria

```text
new project has useful empty state
engine offline state is clear
review workspace remains usable at 5k rows
keyboard shortcuts are documented in UI
all critical actions have visible result
```

---

## 13. M10: Deployment hardening

### Goal

Prepare local Docker and production deployment shape.

### Deliverables

```text
Docker Compose local setup
Kamal draft config
private engine network
secrets documentation
boot checks
backup notes
runbook skeleton
```

### Acceptance criteria

```text
only Rails exposes public port locally by default
engine services reachable from Rails only
Rails refuses missing USAGI_API_SHARED_SECRET in production
API refuses unsigned auth mode in production
smoke test can create/import/review/export
```

---

## 14. Cross-milestone test gates

Every milestone must pass:

```text
bin/rails test
bin/rails test:system for relevant workflows
rubocop or equivalent style gate if configured
security signing tests once M4 exists
contract fixture tests once M4 exists
```

---

## 15. First implementation branch plan

Suggested branches:

```text
rails/m0-scaffold
rails/m1-domain-model
rails/m2-import-workflow
rails/m3-review-workspace
rails/m4-engine-clients-security
rails/m5-manual-hybrid-search
rails/m6-hybrid-auto-suggest
rails/m7-drug-mapper-auto-suggest
rails/m8-exports
rails/m9-polish
rails/m10-deployment
```

Small branches. Small PRs. Small blast radius. A rare adult decision.
