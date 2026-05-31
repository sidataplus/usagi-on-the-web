# Rails App Manual

Status: draft v0.1  
Audience: Rails developers and product workflow operators

## Purpose

`apps/web` is the Rails 8 product workflow layer for Usagi-on-the-Web v3.

```text
Browser
  -> Rails
    -> usagi-api
```

Rails owns users, projects, imports, source terms, mappings, candidates, audit
events, comments, exports, and engine job mirrors. It does not own engine
artifacts or model/index internals.

The product UI is desktop-first. It is intended for reviewer workstations, not
mobile usage, so implementation should prioritize dense tables, keyboard
workflow, and clear job status over responsive mobile breakpoints.

## Local Setup

```bash
cd apps/web
bundle install
bin/rails db:migrate
bin/rails db:seed
bin/rails server
```

The seeded local account is:

```text
demo@usagi.test / password123
```

Development and test use SQLite under `apps/web/storage/`. Production uses
PostgreSQL through `DATABASE_URL` for the primary Rails product database.

## Boundaries

Rails must call engine services only through:

```text
apps/web/app/services/engine_clients/
```

Do not read:

```text
catalog.sqlite
jobs.sqlite
Tantivy indexes
USearch indexes
Tachiom indexes
THIRAWAT model artifacts
engine result files
```

Rails may persist API response DTOs, candidate scores, provenance, request IDs,
and user-facing job state.
