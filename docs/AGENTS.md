# AGENTS.md

Docs-local instructions for agents working under `docs/`.

Root `../AGENTS.md` is the canonical project guide. Follow it first for the
Rails-stage architecture rules, API boundaries, testing expectations, and
human-approval checkpoints.

## Docs Source Order

When editing docs, keep these sources aligned:

1. `../AGENTS.md`
2. `docs/rails/implementation-spec.md`
3. `docs/rails/00_development-plan.md`
4. `docs/rails/01_product-shape.md`
5. `docs/rails/02_domain-model.md`
6. `docs/rails/03_routes-controllers-views.md`
7. `docs/security/rails-api-boundary.md`
8. `docs/testing/contract-fixtures.md`
9. `docs/api/implementation-spec.md`
10. `docs/api/endpoints.md`
11. `docs/api/artifacts.md`
12. `docs/api/jobs.md`
13. `docs/api/rails-integration-contract.md`

## Docs Editing Rules

- Keep root `AGENTS.md` as the single canonical agent guide.
- Keep Rails product behavior in `docs/rails/`.
- Keep Rails-to-engine security rules in `docs/security/`.
- Keep Rails fixture and test-contract rules in `docs/testing/`.
- Keep engine service, artifact, endpoint, and job contracts in `docs/api/`.
- Do not duplicate long sections across docs unless the duplicate is explicitly
  a summary with a link to the detailed source.
- If a doc change shifts service boundaries, Rails data ownership, runtime
  storage, artifact formats, translation timing, or browser-to-engine access,
  ask the human before editing.
