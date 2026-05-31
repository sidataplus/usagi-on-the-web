# Rails Product Shape

Status: draft v0.1  
Scope: product shape, information architecture, visual direction, interaction design

---

## 0. Goal

Define what the Rails app should feel like before development turns it into a hallway of unrelated CRUD screens.

The product should feel like a focused workbench:

```text
simple places
clear objects
review-centered flows
friendly copy
server-rendered pages
small bits of dynamic behavior
```

---

## 1. Product principles

### 1.1 Enough, but not too much

Build the smallest durable product that supports real mapping work.

Include:

```text
projects
imports
mapping review
candidate suggestions
comments
audit
exports
engine diagnostics
```

Exclude from MVP:

```text
generic dashboard builder
real-time collaboration
multi-pane analytics suite
LLM assistant
translation workflow
custom report designer
project bundle import/export
```

### 1.2 Durable places, not configurable chaos

Primary places:

```text
Home / Projects
Project Overview
Import
Review
Jobs
Exports
Settings
Admin Engine Status
```

Everything should have a home. If a feature does not have a clear home, it is probably not ready.

### 1.3 Review is the center

The app exists to help reviewers decide mappings. Search, mapper jobs, exports, comments, and audit all orbit that job.

Do not optimize the product around engine cleverness. The engine suggests. Reviewers decide.

---

## 2. Information architecture

```text
Projects
  Project
    Overview
    Import
    Review mappings
      Mapping row drawer
        Candidates
        Manual search
        Concept detail
        Comments
        Audit
    Jobs
    Exports
    Settings

Admin
  Engine status
```

### 2.1 Home / Projects

Purpose:

```text
show user's projects
create a new project
resume recent work
```

Avoid turning this into a dashboard with pretend metrics.

### 2.2 Project overview

Purpose:

```text
show current project state
show what needs attention
provide obvious next actions
```

Cards:

```text
Source terms
Review progress
Latest suggestions
Latest import
Latest export
Engine readiness
```

Primary actions:

```text
Import source terms
Review mappings
Suggest candidates
Export
```

### 2.3 Import

Purpose:

```text
bring source terms into the project safely
```

Screens:

```text
Upload
Preview
Column mapping
Import progress/result
```

### 2.4 Review

Purpose:

```text
filter source terms
inspect candidates
apply targets
approve/flag/invalidate
comment and audit decisions
```

Main areas:

```text
filter bar
review table
candidate drawer
bulk action bar
```

### 2.5 Jobs

Purpose:

```text
show import/suggest/export progress and failures
```

Avoid exposing engine internals beyond useful provenance and errors.

### 2.6 Exports

Purpose:

```text
create and download usable mapping outputs
```

Screens:

```text
export form
export history
export status
```

### 2.7 Admin Engine Status

Purpose:

```text
show whether catalog/search/mapper are ready
show artifact IDs and vocabulary version
show actionable errors
```

This is an admin place, not a reviewer distraction.

---

## 3. Objects and affordances

| Object | What users do with it |
|---|---|
| Project | create, open, configure, archive |
| Import | upload, preview, run, inspect errors |
| Source term | review, search, map |
| Mapping | apply target, approve, flag, invalidate, comment |
| Candidate | inspect, compare, apply |
| Engine job | watch progress, inspect failure, retry when safe |
| Export | create, download, inspect status |
| Audit event | inspect provenance |

---

## 4. Basecamp-ish UI direction

Use:

```text
plain HTML shapes
soft cards
large readable labels
clear headings
friendly empty states
human copy
few primary actions per page
obvious buttons
server-rendered forms
```

Avoid:

```text
left nav with 40 items
dark enterprise chrome
icon soup
micro text
table controls that require archaeology
configurable dashboards
loading skeleton overuse
fake progress bars for unknown work
```

---

## 5. Visual system

### 5.1 Layout primitives

Use these primitives:

```text
.page
.page-header
.page-actions
.stack
.cluster
.card
.card-grid
.panel
.table-frame
.drawer
.empty-state
.notice
```

### 5.2 Spacing

Use generous spacing:

```css
--space-1: 0.25rem;
--space-2: 0.5rem;
--space-3: 0.75rem;
--space-4: 1rem;
--space-6: 1.5rem;
--space-8: 2rem;
--space-12: 3rem;
```

### 5.3 Color

Keep colors restrained.

Suggested roles:

```text
background
text
muted text
card background
border
selected row
link
good
warning
bad
```

No rainbow severity zoo. Four semantic states are enough for now.

### 5.4 Typography

Use system fonts.

Hierarchy:

```text
h1: page identity
h2: card/section identity
h3: nested section
body: readable default
small: supporting metadata
```

Do not make reviewers squint at 12px metadata while doing clinical mapping work. We have enough medical errors already.

---

## 6. Copy tone

Copy should be:

```text
clear
brief
human
specific
action-oriented
```

Avoid:

```text
Enterprise Resource Planning voice
vague technical errors
sarcastic product copy
blamey validation text
```

### 6.1 Empty state examples

No projects:

```text
No projects yet.
Create a project to import source terms and start mapping.
```

No source terms:

```text
No source terms yet.
Import a CSV or XLSX file to start reviewing mappings.
```

No candidates:

```text
No candidates yet.
Run suggestions or search manually from this row.
```

Engine offline:

```text
The mapping engine is offline.
You can keep reviewing existing mappings, but search and suggestions are unavailable.
```

Import failed:

```text
The file could not be imported.
Check the highlighted rows and try again.
```

---

## 7. Progress metaphor

Use real states instead of fake percentages when work is not measurable.

For imports/exports/batch jobs, use actual counts:

```text
2,400 of 5,000 rows processed
3 failed
stage: reranking candidates
```

For product progress, use counts and categories:

```text
Unchecked
Approved
Flagged
Invalid
```

If desired later, a Hill Chart-like project status can summarize:

```text
Unknown -> Figuring out -> Reviewing -> Done
```

Do not pretend model confidence is project progress. That is numerology wearing a lab coat.

---

## 8. Page sketches

### 8.1 Projects index

```text
+------------------------------------------------------+
| Usagi-on-the-Web                         New Project |
+------------------------------------------------------+
| Your projects                                        |
|                                                      |
| [ Medication source mapping          4,200 mappings ]|
| [ Lab mappings                          800 mappings ]|
|                                                      |
| Empty: No projects yet. Create one to begin.         |
+------------------------------------------------------+
```

### 8.2 Project overview

```text
+------------------------------------------------------+
| Medication source mapping                 Review     |
| Drug domain / RxNorm / ATHENA 20250827               |
+------------------------------------------------------+
| [ Source terms 4,200 ] [ Approved 1,300 ] [ Flagged ]|
|                                                      |
| Next steps                                           |
| - Review unchecked mappings                          |
| - Run drug suggestions                               |
| - Export approved rows                               |
+------------------------------------------------------+
```

### 8.3 Review workspace

```text
+------------------------------------------------------+
| Review mappings                                      |
| [search] [status] [domain] [suggest] [export]        |
+------------------------------+-----------------------+
| table                        | candidate drawer      |
| source | target | status     | selected mapping      |
| ...                          | candidates            |
|                              | comments              |
|                              | audit                 |
+------------------------------+-----------------------+
```

---

## 9. Interaction rules

### 9.1 Server-first

Default interaction:

```text
form submit -> Rails action -> HTML/Turbo response
```

### 9.2 Turbo when page flow benefits

Use Turbo for:

```text
candidate drawer
search result replacement
job status cards
mapping row replacement
flash messages
```

### 9.3 Stimulus only for local behavior

Stimulus controllers:

```text
row selection
keyboard shortcuts
drawer open/close
file upload preview
auto-submit filters
copy-to-clipboard
```

Do not build a client-side mapping state store. Rails owns the state.

---

## 10. Navigation

Use sparse navigation.

Top-level:

```text
Projects
Admin
Account
```

Project-level tabs:

```text
Overview
Import
Review
Jobs
Exports
Settings
```

Avoid nested nav unless a page truly needs it.

---

## 11. Status language

Mapping statuses:

| Status | Label | Meaning |
|---|---|---|
| `UNCHECKED` | Unchecked | Needs reviewer decision |
| `APPROVED` | Approved | Reviewer accepted mapping |
| `FLAGGED` | Flagged | Needs attention or second pass |
| `INVALID` | Invalid | Should not be mapped/exported as approved |

Engine job states:

| State | Label |
|---|---|
| `queued` | Waiting |
| `running` | Running |
| `succeeded` | Finished |
| `succeeded_with_errors` | Finished with errors |
| `failed` | Failed |
| `cancelled` | Cancelled |

---

## 12. Accessibility baseline

Required:

```text
semantic headings
labels for inputs
buttons with text
visible focus states
keyboard-accessible drawer
keyboard-accessible table actions
sufficient color contrast
errors linked to fields where possible
```

Mapping review is already cognitively heavy. The UI should not add a maze on top like some kind of punishment garden.
