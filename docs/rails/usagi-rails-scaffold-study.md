# usagi-rails Scaffold Study

Date: 2026-05-31  
Source repo studied: `/Users/na399/GitHub/sidataplus/usagi-rails`  
Compared against: `docs/rails/`, `docs/security/`, `docs/testing/`, and `docs/api/`  
Last updated after lift into `apps/web`: 2026-05-31

## Lift Status

The useful `usagi-rails` scaffold pieces have now been lifted into
`apps/web`. Treat `/Users/na399/GitHub/sidataplus/usagi-rails` as a read-only
historical scaffold reference from this point forward. Rails development should
continue in this repository under `apps/web`.

## Summary

`usagi-rails` started as a Rails product/UI scaffold, not as a domain model or
API integration source of truth. It was patched in place with a Rails-owned
workflow skeleton and a target-shaped engine client seam, then lifted into
`apps/web` as the seed for the Rails product layer.

The strongest reusable pieces are the Rails 8 app shell, importmap/Hotwire setup,
custom CSS system, mapping review table/drawer patterns, small Stimulus
controllers, demo seeds, CSV export shape, request/controller smoke tests, the
new `EngineClients::*` adapter seam, and the new
import/source-term/candidate/job/export/audit model skeleton.

The lifted `apps/web` app has been reconciled toward the current
Usagi-on-the-Web v3 Rails spec: core records use prefixed string IDs, mappings
are source-term-backed, sessions replace the demo `current_user`, project roles
are enforced, and `EngineClients::*` provides both offline stubs and signed HTTP
transport.

The current Rails app now has a tested path for import, source terms, manual
search, auto-suggest job creation, candidate application without auto-approval,
engine job history, audit events, admin engine status, and CSV export records.

## What Was Lifted

- **Rails 8 app setup:** Gemfile choices are aligned with the docs: Rails 8.1,
  Propshaft, importmap, Turbo, Stimulus, Solid Queue/Cache/Cable, Kamal, Thruster,
  Capybara/Selenium, Brakeman, Bundler Audit, and RuboCop Omakase.
- **Application shell:** The layout, sidebar/topbar/shared partials, lucide icon
  usage, top-level `turbo_frame_tag "modal"`, and responsive left-nav pattern are
  good starting points for `apps/web`.
- **CSS direction:** `tokens.css`, `base.css`, `layout.css`, `components.css`,
  and `mappings.css` are close to the desired custom-CSS, server-rendered product
  style. Split them into the target files named in `AGENTS.md` rather than copying
  the five-file grouping unchanged.
- **Stimulus controllers:** `autosubmit`, `bulk_select`, `dropdown`, `keyboard`,
  `modal`, `sidebar`, and `dismiss` match expected Rails sprinkles. Drop the demo
  `hello_controller`.
- **Mapping review UX:** The project mapping index, Turbo-frame table refresh,
  status chips, bulk status toolbar, sortable headers, row actions, modal detail
  drawer, previous/next navigation, candidate list, manual search partial, history
  panel, and comments panel are directly relevant.
- **Small UI helpers:** `UiHelper` patterns for status badges, project badges,
  score badges, and nav active-state classes can be adapted to the target model
  names.
- **Demo seeds:** The deterministic demo users/projects/mappings, fixed RNG,
  condition/drug source pools, status mix, and history/comments seeding are useful
  for local visual development.
- **CSV export sketch:** The simple CSV export controller is a useful first
  reference for export UI and response shape, though the target app needs export
  records/jobs.
- **Integration tests:** `test/integration/pages_test.rb` is a good smoke-test
  template for top-level pages, Turbo frames, status updates, bulk updates,
  comments, concept search, candidate partials, exports, settings, and 404s.
- **Engine client seam:** `app/services/engine_clients/` now contains
  `BaseClient`, `CatalogClient`, `SearchClient`, `MapperClient`, and
  `StubTransport`. The seam mirrors catalog/search/mapper boundaries while
  keeping deterministic local behavior for tests and UI work.
- **Engine error handling shape:** `EngineClients::BaseClient::Error` preserves
  code, message, details, request ID, and HTTP status from the expected API error
  envelope.
- **Rails workflow skeleton:** New models now cover `ImportBatch`, `SourceTerm`,
  `MappingCandidate`, `EngineJob`, `MappingExport`, and `AuditEvent`. These are
  useful as the Rails-owned product records around the API engine.
- **Stubbed vertical workflow:** Project imports can create source terms and
  review mappings; manual search and auto-map persist candidates via
  `EngineClients`; persisted candidates can be applied into mappings; CSV export
  creates `MappingExport` and `AuditEvent` records.
- **Prefixed ID pattern for new records:** `HasPrefixedId` gives new workflow
  tables string IDs such as `imp_`, `src_`, `cand_`, `engjob_`, `exp_`, and
  `audit_`. This is narrower than the final target ID system but moves the
  scaffold away from pure integer assumptions.

## What Not To Lift Directly

- **Legacy schema shape:** The original `projects`, `mappings`, comments, events,
  users, and memberships still use Rails integer IDs. `mappings` still embeds
  source fields, uses `version` instead of `lock_version`, and is not yet rebuilt
  around `source_terms`.
- **Authentication:** `ApplicationController#current_user` resolves a demo user.
  The target needs real Rails-owned authentication and session routes.
- **Authorization:** Admin gating exists, but project membership authorization is
  not complete enough for the target permission model.
- **Live engine integration:** `EngineClients::StubTransport` and sample concepts
  are wireframe-only. They should be replaced or supplemented by signed,
  request-id-aware HTTP transports once the API contracts are stable.
- **Routes:** The scaffold routes are useful for UI intuition but do not fully
  match the target route map. Sessions and admin engine status/build routes are
  still missing; imports, auto-map, engine jobs, manual search, and candidate
  application now exist in scaffold form.
- **Admin/dashboard surface:** Current admin pages are generic user/audit/dashboard
  screens. The target first needs engine status/build admin, not a broad admin
  dashboard.
- **PWA/static marketing pages:** Welcome/privacy/terms/guest/PWA files are not
  core to the Rails MVP.

## Missing For Target Rails Dev

- **Full ID system:** New workflow records have prefixed string IDs, but legacy
  records still use integer IDs. Add a consistent `Ids.generate(prefix)` or
  equivalent for `usr_`, `proj_`, `pmem_`, `map_`, `com_`, and any renamed target
  records.
- **Auth/session layer:** Implement real `User` authentication, session routes,
  `Current.user`, `Current.request_id`, and project authorization helpers.
- **Target domain reconciliation:** The patched scaffold now has the broad
  workflow tables, but names and semantics still need alignment with
  `docs/rails/02_domain-model.md`, especially `ProjectMember` vs.
  `ProjectMembership`, `ImportSession` vs. `ImportBatch`, `Export` vs.
  `MappingExport`, and `Comment` vs. `MappingComment`.
- **Import workflow:** A pasted-CSV import path now creates source terms and
  review mappings. Still missing upload, preview, column mapping, Active Storage
  attachment, `ImportSourceFileJob`, and error-row UX.
- **Candidate workflow:** Manual search and auto-map now persist candidates, and
  candidate application copies a selected candidate into the mapping. Still
  missing richer candidate provenance UI and conflict handling.
- **HTTP engine transport:** `app/services/engine_clients/` exists, but it still
  needs base URL config, request IDs, signed headers, JSON serialization, timeout
  handling, safe GET retries, structured logging, idempotency keys, and a
  `JobsClient`.
- **API security:** Implement the Rails signing side from
  `docs/security/rails-api-boundary.md`; the old scaffold has no HMAC signing.
- **Background jobs:** Add `StartAutoMapJob`, `RunHybridSearchJob`,
  `PollEngineJobJob`, `PersistMapperResultsJob`,
  `PersistHybridSearchResultsJob`, and `CreateExportJob`.
- **Engine job UI:** Project-scoped engine job index/show pages now exist. Still
  missing progress cards, live polling, error-state detail, result artifacts, and
  retry/cancel handling where safe.
- **Manual search flow:** `ManualSearchesController#create` now calls the engine
  client seam, persists returned candidates, writes audit events, and replaces
  candidate lists via Turbo Stream. Still needs signed HTTP transport and
  contract fixtures.
- **Auto-map flow:** Import-scoped auto-map now chooses mapper batch behavior for
  Drug/RxNorm projects and hybrid search behavior otherwise, using the stub
  engine seam. Still needs background job execution and live engine job polling.
- **Export workflow:** CSV export now creates `MappingExport` and `AuditEvent`
  records. Still missing background export jobs, stored downloadable artifacts,
  and non-CSV formats.
- **Contract fixtures:** Add fixtures and tests from `docs/testing/contract-fixtures.md`
  so Rails tests do not depend on a live engine.
- **Production database shape:** The source scaffold uses SQLite in production.
  The target docs prefer PostgreSQL for production, with SQLite acceptable only
  for dev/test.

## Key Mismatches To Resolve Before Any Lift

| Area | `usagi-rails` | Target docs |
|---|---|---|
| IDs | Mixed: legacy integer IDs plus prefixed string IDs for new workflow records | String IDs with prefixes throughout |
| Source terms | Separate `source_terms` added, but `mappings` still embeds legacy source fields | Separate `source_terms` as the source of truth |
| Candidates | Manual search and auto-map persist candidates; fallback stub candidates remain for empty states | Persisted `mapping_candidates` everywhere |
| Jobs | `engine_jobs` mirror table and history UI added; background jobs not yet built | Import, auto-map, polling, persistence, export jobs |
| Engine client | `EngineClients::*` seam with local stub transport | Signed `EngineClients::*` HTTP clients |
| Auth | Demo user lookup | Rails-owned authentication |
| Project roles | `project_memberships` with owner/editor/viewer/guest | `project_members` with owner/admin/reviewer/observer |
| Audit | `audit_events` added; legacy `mapping_events` still exists | First-class `audit_events` plus comments |
| Imports | Pasted CSV import creates source terms and review mappings; no upload/preview UI yet | Upload, preview, import session, source-term creation |
| Exports | CSV response creates `mapping_exports`; no artifact storage yet | Export records/jobs/downloads |
| Admin | Generic dashboard/users/audit | Engine status/builds first |

## Remaining Rails Hardening Order

1. Keep development inside `apps/web`; use `usagi-rails` only for read-only
   comparison if a UI pattern is unclear.
2. Replace pasted CSV import with upload/preview/column mapping while preserving
   the current `ImportBatch` and `SourceTerm` contract.
3. Add background jobs for import parsing, auto-map, engine polling, candidate
   persistence, and export generation.
4. Add signed HTTP transports, a real `JobsClient`, and contract fixtures after
   endpoint DTOs stabilize.
5. Reconcile names and IDs with the final target domain model before treating the
   scaffold as production Rails code.
6. Add real auth/session/project authorization and remove demo `current_user`.

## Rails-Only Patch Notes

The following changes were made directly in `/Users/na399/GitHub/sidataplus/usagi-rails`:

- Added `app/services/engine_clients/` with catalog/search/mapper clients,
  local stub transport, and error-envelope handling.
- Updated `UsagiApi::Client` to remain a compatibility facade over
  `EngineClients::*`.
- Added `ImportBatch`, `SourceTerm`, `MappingCandidate`, `EngineJob`,
  `MappingExport`, `AuditEvent`, and `HasPrefixedId`.
- Added migrations `20260531100007` through `20260531100012`.
- Updated `Project` and `User` associations for the new workflow records.
- Added service and model tests for the engine seam and workflow skeleton.
- Added pasted-CSV imports, source-term-backed mappings, manual search candidate
  persistence, import auto-map candidate persistence, candidate application,
  engine job pages, and CSV export records/audit.

## Verification Notes

Original study was read-only. After the Rails-only scaffold patches, verification
was run inside `/Users/na399/GitHub/sidataplus/usagi-rails`:

```text
ruby -c touched service/model files: OK
bundle install: OK, installed locked lucide-rails 0.7.4
bin/rails db:migrate: OK
bin/rails test test/services/engine_clients_test.rb: 5 runs, 16 assertions, 0 failures
bin/rails test test/models/engine_workflow_models_test.rb: 4 runs, 20 assertions, 0 failures
bin/rails test test/controllers/workflow_controllers_test.rb: 8 runs, 41 assertions, 0 failures
bin/rails test: 29 runs, 122 assertions, 0 failures
```

The `usagi-rails` scaffold is still an all-untracked git tree, so `git status`
reports broad `??` entries rather than a clean file-level diff.
