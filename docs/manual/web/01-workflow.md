# Rails Workflow Manual

Status: draft v0.1  
Audience: Rails developers and reviewers

## Product Flow

The MVP workflow is:

1. Sign in.
2. Create or open a project.
3. Import source terms from CSV, TSV, pasted rows, or XLSX.
4. Review source-term mappings.
5. Run manual search or project-level auto-suggest.
6. Apply a candidate to fill target concept fields.
7. Approve, flag, or invalidate the mapping manually.
8. Export review, source-to-concept-map, candidate, or audit CSVs.

Applying a candidate does not approve the mapping. Approval remains an explicit
human review action.

## Core Tables

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

Use string IDs with the canonical prefixes from `AGENTS.md`.

## Roles

Project membership roles are:

| Role | Import | Suggest | Review | Export | Manage |
|---|---:|---:|---:|---:|---:|
| owner | yes | yes | yes | yes | yes |
| admin | yes | yes | yes | yes | yes |
| reviewer | no | yes | yes | yes | no |
| observer | no | no | no | no | no |

Site admins bypass project membership checks.

