# Rails Routes, Controllers, and Views

Status: draft v0.1  
Scope: HTTP routes, controller responsibilities, Turbo Frames/Streams, view structure

---

## 0. Goal

Define the Rails web surface without accidentally building a JSON API for our own pages like it is 2016 and everyone has lost judgment.

Default pattern:

```text
Browser submits HTML form
  -> Rails controller
  -> service object if needed
  -> HTML or Turbo Stream response
```

---

## 1. Route map

```ruby
Rails.application.routes.draw do
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
end
```

---

## 2. ApplicationController

Responsibilities:

```text
authenticate user
set Current.user
set Current.request_id
authorize project access helpers
handle not found
handle authorization errors
handle engine client errors
```

Example concerns:

```ruby
class ApplicationController < ActionController::Base
  before_action :set_request_id
  before_action :authenticate_user!

  rescue_from EngineClients::BaseClient::Error, with: :handle_engine_error
  rescue_from ActiveRecord::RecordNotFound, with: :handle_not_found

  private

  def set_request_id
    Current.request_id = request.request_id
  end
end
```

---

## 3. SessionsController

Routes:

```text
GET    /session/new
POST   /session
DELETE /session
```

Views:

```text
views/sessions/new.html.erb
```

Responsibilities:

```text
render login
create session
destroy session
```

MVP can use email/password.

---

## 4. ProjectsController

Routes:

```text
GET  /projects
POST /projects
GET  /projects/:id
PATCH /projects/:id
```

Actions:

| Action | Responsibility |
|---|---|
| `index` | list visible projects, render new project form |
| `create` | create project and owner membership |
| `show` | project overview |
| `update` | update project settings/status |

Views:

```text
views/projects/index.html.erb
views/projects/show.html.erb
views/projects/_project_card.html.erb
views/projects/_form.html.erb
views/projects/_overview_counts.html.erb
views/projects/_next_steps.html.erb
views/projects/_project_nav.html.erb
```

Project overview sections:

```text
summary counts
review status
latest import
latest suggestions job
latest export
engine readiness summary
next actions
```

---

## 5. ImportSessionsController

Routes:

```text
GET  /projects/:project_id/import_sessions
GET  /projects/:project_id/import_sessions/new
POST /projects/:project_id/import_sessions/preview
POST /projects/:project_id/import_sessions
POST /projects/:project_id/import_sessions/:id/confirm
GET  /projects/:project_id/import_sessions/:id
```

Actions:

| Action | Responsibility |
|---|---|
| `index` | import history |
| `new` | upload form |
| `create` | attach file, detect columns or enqueue import depending step |
| `preview` | attach file, detect columns, render preview and column mapping |
| `confirm` | import previewed rows using confirmed column mapping |
| `show` | import status, errors, summary |

Suggested flow:

```text
new upload
  -> create import_session with file
  -> show preview form
  -> confirm column mapping
  -> import rows
  -> show import status
```

Views:

```text
views/import_sessions/new.html.erb
views/import_sessions/show.html.erb
views/import_sessions/_form.html.erb
views/import_sessions/_preview.html.erb
views/import_sessions/_column_mapping.html.erb
views/import_sessions/_status_card.html.erb
views/import_sessions/_error_rows.html.erb
```

Turbo Frames:

```text
import_preview
import_status
```

---

## 6. MappingsController

Routes:

```text
GET   /projects/:project_id/mappings
GET   /mappings/:id
PATCH /mappings/:id
POST  /mappings/:id/apply_candidate
POST  /mappings/:id/approve
POST  /mappings/:id/flag
POST  /mappings/:id/invalidate
POST  /mappings/bulk_update
```

Actions:

| Action | Responsibility |
|---|---|
| `index` | review table with filters/sorts/pagination |
| `show` | candidate drawer |
| `update` | target/status updates |
| `apply_candidate` | apply selected candidate target fields |
| `approve` | set `APPROVED` |
| `flag` | set `FLAGGED` |
| `invalidate` | set `INVALID` |
| `bulk_update` | selected status updates |

Views:

```text
views/mappings/index.html.erb
views/mappings/show.html.erb
views/mappings/_filters.html.erb
views/mappings/_table.html.erb
views/mappings/_row.html.erb
views/mappings/_bulk_bar.html.erb
views/mappings/_candidate_drawer.html.erb
views/mappings/_current_target.html.erb
views/mappings/_status_badge.html.erb
views/mappings/_empty.html.erb
```

Turbo Frames:

```text
mappings_table
candidate_drawer
mapping_<id>
bulk_bar
flash
```

Index params:

```text
q
status
domain_id
vocabulary_id
has_target
has_candidates
sort
page
per_page
```

Sort values:

```text
frequency_desc
source_code_asc
updated_desc
score_desc
status_asc
```

---

## 7. MappingCandidatesController

Routes:

```text
GET /mappings/:mapping_id/mapping_candidates
```

Responsibilities:

```text
list persisted candidates for a mapping
render candidate list partial
support Turbo drawer refresh
```

Views:

```text
views/mapping_candidates/index.html.erb
views/mapping_candidates/_list.html.erb
views/mapping_candidates/_candidate.html.erb
views/mapping_candidates/_score_stack.html.erb
views/mapping_candidates/_provenance.html.erb
```

Candidate card should show:

```text
rank
concept name
concept ID
vocabulary/domain/class
method
final/rrf score
warnings
apply button
provenance disclosure
```

---

## 8. ManualSearchesController

Route:

```text
POST /mappings/:mapping_id/manual_search
```

Responsibilities:

```text
validate query
call EngineClients::SearchClient.search_concepts
persist returned candidates
write audit event
replace candidate list in drawer
```

Request params:

```text
q
limit
domain_id optional
vocabulary_id optional
```

Response:

```text
Turbo Stream replacing candidate list and flash
HTML redirect fallback
```

Failure behavior:

```text
show inline engine error in drawer
preserve mapping page
include request_id if available
```

---

## 9. AutoMapsController

Route:

```text
POST /projects/:project_id/auto_map
```

Responsibilities:

```text
choose strategy by project domain
create engine_jobs mirror
trigger background job
render/redirect to job card
```

Strategy:

```ruby
if project.drug_domain?
  StartAutoMapJob.perform_later(project.id)
else
  RunHybridSearchJob.perform_later(project.id)
end
```

Response:

```text
Turbo Stream prepend job card
HTML redirect to project jobs page
```

---

## 10. EngineJobsController

Routes:

```text
GET /projects/:project_id/engine_jobs
GET /projects/:project_id/engine_jobs/:id
```

Responsibilities:

```text
job history
job detail
progress card rendering
failure display
retry link where safe
```

Views:

```text
views/engine_jobs/index.html.erb
views/engine_jobs/show.html.erb
views/engine_jobs/_job_card.html.erb
views/engine_jobs/_progress.html.erb
views/engine_jobs/_error.html.erb
```

Turbo Streams:

```text
engine_job_<id>
project_engine_jobs
```

---

## 11. ExportsController

Routes:

```text
GET  /projects/:project_id/exports
POST /projects/:project_id/exports
GET  /projects/:project_id/exports/:id
```

Responsibilities:

```text
show export history
create export job
show export status/download
```

Views:

```text
views/exports/index.html.erb
views/exports/show.html.erb
views/exports/_form.html.erb
views/exports/_export_card.html.erb
views/exports/_format_option.html.erb
```

Export format options:

```text
USAGI CSV
SOURCE_TO_CONCEPT_MAP CSV
Review CSV
Candidate JSONL
Candidate CSV
Audit CSV
```

---

## 12. CommentsController

Routes:

```text
GET  /mappings/:mapping_id/comments
POST /mappings/:mapping_id/comments
```

Responsibilities:

```text
list comments in drawer
create comment
write audit event
Turbo update comment list
```

Views:

```text
views/comments/index.html.erb
views/comments/_comment.html.erb
views/comments/_form.html.erb
```

---

## 13. Admin::EngineStatusController

Route:

```text
GET /admin/engine_status
```

Responsibilities:

```text
call catalog/search/mapper status endpoints
show readiness cards
show artifact IDs
show cached last-known status when unavailable
render errors without crashing
```

Views:

```text
views/admin/engine_status/show.html.erb
views/admin/engine_status/_catalog_card.html.erb
views/admin/engine_status/_search_card.html.erb
views/admin/engine_status/_mapper_card.html.erb
views/admin/engine_status/_error_card.html.erb
```

---

## 14. Admin::EngineBuildsController

Route:

```text
POST /admin/engine_builds
```

MVP status:

```text
optional / later in admin UI
```

Build buttons should not ship until the core review workflow works. Humans already have enough buttons that start expensive jobs without understanding them.

---

## 15. Layout

Application layout:

```text
skip link
header
  product name
  primary nav
  account nav
main
  flash
  content
footer optional
```

Project layout partial:

```text
project header
project nav tabs
content
```

---

## 16. Turbo response examples

### Apply candidate

```erb
<%= turbo_stream.replace dom_id(@mapping), partial: "mappings/row", locals: { mapping: @mapping } %>
<%= turbo_stream.replace "candidate_drawer", partial: "mappings/candidate_drawer", locals: { mapping: @mapping } %>
<%= turbo_stream.replace "flash", partial: "layouts/flash", locals: { notice: "Candidate applied" } %>
```

### Manual search

```erb
<%= turbo_stream.replace "candidate_list", partial: "mapping_candidates/list", locals: { mapping: @mapping, candidates: @candidates } %>
<%= turbo_stream.replace "flash", partial: "layouts/flash", locals: { notice: "Search finished" } %>
```

### Job update

```erb
<%= turbo_stream.replace dom_id(@engine_job), partial: "engine_jobs/job_card", locals: { engine_job: @engine_job } %>
```

---

## 17. Stimulus controllers

### `row_selection_controller`

Responsibilities:

```text
select/unselect row checkboxes
select all visible
clear selection
update bulk bar count
```

No persistence. Form submit sends selected IDs.

### `keyboard_shortcuts_controller`

Responsibilities:

```text
j/k row movement
Enter open selected row
Esc close drawer
a approve
f flag
x invalid
/ focus search
```

### `drawer_controller`

Responsibilities:

```text
open drawer
close drawer
manage focus
restore focus on close
```

### `auto_submit_controller`

Responsibilities:

```text
submit filter form on select change
debounce text input if needed
```

### `file_upload_preview_controller`

Responsibilities:

```text
show chosen file name
show file size
basic client-side extension hint
```

---

## 18. View partial conventions

Use names by object and role:

```text
_projects/project_card
_mappings/row
_mappings/table
_mapping_candidates/candidate
_engine_jobs/job_card
_exports/export_card
```

Partials should accept locals explicitly. Avoid relying on instance variables inside partials unless the partial is page-specific.

---

## 19. Error rendering

Every engine-dependent frame should have an error partial.

Example:

```text
Search unavailable
The mapping engine could not be reached.
Request ID: req_abc
```

Do not crash the whole review page because search is offline. That would be like canceling a hospital because the coffee machine is broken.

---

## 20. Pagination

Use server-side pagination for mappings.

Default:

```text
per_page = 100
max_per_page = 250
```

Do not ship a custom virtual table in MVP. PostgreSQL and pagination are enough for 5k rows. We are not indexing the Library of Babel.
