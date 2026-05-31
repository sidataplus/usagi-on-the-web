# Rails Domain Model

Status: draft v0.1  
Scope: Rails-owned product data model, validations, associations, lifecycle rules

---

## 0. Boundary

Rails owns product workflow state.

`usagi-api` owns engine artifacts and engine job internals.

Rails stores engine outputs and provenance, but not engine index/model internals.

---

## 1. Core entities

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

Relationship sketch:

```text
User
  has many ProjectMembers
  has many Projects through ProjectMembers

Project
  has many ProjectMembers
  has many ImportSessions
  has many SourceTerms
  has many Mappings
  has many MappingCandidates
  has many EngineJobs
  has many Exports
  has many AuditEvents
  has many Comments

SourceTerm
  belongs to Project
  belongs to ImportSession optional
  has one Mapping

Mapping
  belongs to Project
  belongs to SourceTerm
  has many MappingCandidates
  has many Comments
  has many AuditEvents
```

---

## 2. ID generation

Use string IDs with prefixes.

```ruby
module Ids
  PREFIXES = {
    user: "usr",
    project: "proj",
    project_member: "pmem",
    import_session: "imp",
    source_term: "src",
    mapping: "map",
    mapping_candidate: "cand",
    engine_job: "ejob",
    export: "exp",
    audit_event: "aud",
    comment: "com"
  }.freeze

  def self.generate(prefix)
    "#{prefix}_#{SecureRandom.uuid}"
  end
end
```

Each model should assign ID before validation if absent.

---

## 3. `users`

### Migration

```ruby
create_table :users, id: :string do |t|
  t.string :email, null: false
  t.string :name, null: false
  t.string :password_digest
  t.boolean :admin, null: false, default: false
  t.datetime :last_seen_at
  t.timestamps
end

add_index :users, :email, unique: true
```

### Validations

```text
email required, unique, normalized lowercase
name required
```

### Notes

MVP authentication can use Rails-native password authentication. OAuth and institutional SSO are later work.

---

## 4. `projects`

### Migration

```ruby
create_table :projects, id: :string do |t|
  t.string :name, null: false
  t.text :description
  t.string :status, null: false, default: "active"

  t.string :mapping_domain, null: false, default: "Drug"
  t.string :source_vocabulary
  t.string :vocabulary_version

  t.jsonb :target_domain_ids, null: false, default: []
  t.jsonb :target_vocabulary_ids, null: false, default: []
  t.jsonb :settings, null: false, default: {}

  t.string :created_by_id, null: false
  t.timestamps
end

add_index :projects, :status
add_index :projects, :mapping_domain
add_index :projects, :created_by_id
```

### Status values

```text
active
archived
completed
```

### Mapping domains

```text
Drug
Condition
Procedure
Measurement
Observation
Device
Mixed
```

### Derived behavior

```ruby
def drug_domain?
  mapping_domain == "Drug"
end

def hybrid_search_domain?
  !drug_domain?
end
```

### Settings shape

```json
{
  "auto_apply_top_candidate": false,
  "auto_apply_threshold": 0.95,
  "candidate_limit": 20,
  "hybrid": {
    "rrf_k": 60,
    "lexical_top_k": 100,
    "sapbert_top_k": 100
  }
}
```

### Lifecycle

```text
project created
  -> owner project_member created
  -> source terms imported
  -> mappings reviewed
  -> exports generated
  -> archived or completed
```

---

## 5. `project_members`

### Migration

```ruby
create_table :project_members, id: :string do |t|
  t.string :project_id, null: false
  t.string :user_id, null: false
  t.string :role, null: false
  t.timestamps
end

add_index :project_members, [:project_id, :user_id], unique: true
add_index :project_members, :user_id
```

### Roles

```text
owner
admin
reviewer
observer
```

### Permissions

| Action | owner | admin | reviewer | observer |
|---|---:|---:|---:|---:|
| view project | yes | yes | yes | yes |
| import source terms | yes | yes | no | no |
| run suggestions | yes | yes | yes | no |
| update mappings | yes | yes | yes | no |
| approve mappings | yes | yes | yes | no |
| export | yes | yes | yes | no |
| manage members | yes | yes | no | no |
| archive project | yes | no | no | no |

---

## 6. `import_sessions`

### Migration

```ruby
create_table :import_sessions, id: :string do |t|
  t.string :project_id, null: false
  t.string :created_by_id, null: false

  t.string :file_name, null: false
  t.string :file_content_type
  t.bigint :file_size

  t.string :state, null: false, default: "pending"
  t.jsonb :column_mapping, null: false, default: {}
  t.jsonb :detected_columns, null: false, default: []
  t.jsonb :summary, null: false, default: {}
  t.jsonb :error, null: false, default: {}

  t.datetime :started_at
  t.datetime :finished_at
  t.timestamps
end

add_index :import_sessions, :project_id
add_index :import_sessions, :state
```

### Attachment

```ruby
has_one_attached :source_file
```

### States

```text
pending
previewed
queued
running
succeeded
failed
cancelled
```

### Column mapping

```json
{
  "source_code": "local_code",
  "source_name": "description",
  "source_frequency": "count",
  "domain_hint": "domain"
}
```

### Summary

```json
{
  "rows_seen": 5000,
  "rows_imported": 4980,
  "rows_failed": 20,
  "duplicate_source_codes": 2
}
```

---

## 7. `source_terms`

### Migration

```ruby
create_table :source_terms, id: :string do |t|
  t.string :project_id, null: false
  t.string :import_session_id

  t.string :source_code, null: false
  t.text :source_name, null: false
  t.integer :source_frequency

  t.string :source_domain_hint
  t.string :source_vocabulary
  t.integer :source_row_number

  t.text :normalized_source_name
  t.jsonb :raw_row, null: false, default: {}

  t.timestamps
end

add_index :source_terms, :project_id
add_index :source_terms, [:project_id, :source_code], unique: true
add_index :source_terms, :source_domain_hint
```

### Validations

```text
project required
source_code required
source_name required
source_code unique within project
source_frequency integer >= 0 if present
```

### Normalization

Use simple Rails-side normalization for filtering/display only:

```text
strip
collapse whitespace
lowercase
```

Do not replicate engine normalization. That belongs in `usagi-api`.

---

## 8. `mappings`

### Migration

```ruby
create_table :mappings, id: :string do |t|
  t.string :project_id, null: false
  t.string :source_term_id, null: false

  t.integer :target_concept_id
  t.text :target_concept_name
  t.string :target_domain_id
  t.string :target_vocabulary_id
  t.string :target_concept_class_id
  t.string :target_standard_concept
  t.string :target_concept_code

  t.float :match_score
  t.string :search_method
  t.string :mapping_status, null: false, default: "UNCHECKED"
  t.string :equivalence

  t.string :reviewed_by_id
  t.datetime :reviewed_at

  t.integer :lock_version, null: false, default: 0
  t.timestamps
end

add_index :mappings, :project_id
add_index :mappings, :source_term_id, unique: true
add_index :mappings, :mapping_status
add_index :mappings, :target_concept_id
add_index :mappings, [:project_id, :mapping_status]
```

### Status values

```text
UNCHECKED
APPROVED
FLAGGED
INVALID
```

### Equivalence values

```text
Equivalent
Narrower
Broader
Related
No match
Unclear
```

### Rules

1. Mapping decisions are Rails truth.
2. Applying a candidate does not automatically approve.
3. Reviewed mappings must not be overwritten by auto-suggest jobs.
4. Updates must use optimistic locking.
5. Status updates must write audit events.

### Candidate application target fields

```text
target_concept_id
target_concept_name
target_domain_id
target_vocabulary_id
target_concept_class_id
target_standard_concept
target_concept_code
match_score
search_method
```

### Status transition guidance

```text
UNCHECKED -> APPROVED
UNCHECKED -> FLAGGED
UNCHECKED -> INVALID
FLAGGED -> APPROVED
FLAGGED -> INVALID
APPROVED -> FLAGGED
APPROVED -> INVALID
INVALID -> FLAGGED
INVALID -> APPROVED
```

All transitions are allowed but audited.

---

## 9. `mapping_candidates`

### Migration

```ruby
create_table :mapping_candidates, id: :string do |t|
  t.string :project_id, null: false
  t.string :mapping_id, null: false
  t.string :source_term_id, null: false

  t.integer :rank, null: false
  t.integer :concept_id, null: false
  t.text :concept_name, null: false
  t.string :domain_id
  t.string :vocabulary_id
  t.string :concept_class_id
  t.string :standard_concept
  t.string :concept_code

  t.float :tantivy_score
  t.float :sapbert_score
  t.float :rrf_score
  t.float :tachiom_maxsim_score
  t.float :bimaxsim_score
  t.float :tie_breaker_score
  t.float :final_score

  t.string :method, null: false
  t.string :candidate_set_id
  t.jsonb :component_ranks, null: false, default: {}
  t.jsonb :features, null: false, default: {}
  t.jsonb :provenance, null: false, default: {}
  t.jsonb :warnings, null: false, default: []

  t.timestamps
end

add_index :mapping_candidates, :project_id
add_index :mapping_candidates, :mapping_id
add_index :mapping_candidates, [:mapping_id, :rank]
add_index :mapping_candidates, [:project_id, :candidate_set_id]
add_index :mapping_candidates, [:mapping_id, :concept_id, :method], unique: true, name: "idx_candidates_unique_method"
```

### Methods

```text
hybrid_rrf
lexical_tantivy
sapbert_cls
thirawat_tachiom_bimaxsim_tiebreak
manual
```

### Provenance examples

Hybrid search:

```json
{
  "catalog_artifact_id": "athena-20250827-standard-v1",
  "tantivy_artifact_id": "athena-20250827-tantivy-v1",
  "sapbert_artifact_id": "athena-20250827-sapbert-cls-v1",
  "api_version": "0.1.0"
}
```

Drug mapper:

```json
{
  "catalog_artifact_id": "athena-20250827-standard-v1",
  "thirawat_model_id": "sidataplus/THIRAWAT-SapBERT",
  "tachiom_artifact_id": "athena-20250827-thirawat-drug-tachiom-v1",
  "api_version": "0.1.0"
}
```

### Deduplication rule

Unique key:

```text
mapping_id + concept_id + method
```

If duplicate:

```text
update rank, scores, component_ranks, features, provenance, warnings
preserve created_at
```

---

## 10. `engine_jobs`

### Migration

```ruby
create_table :engine_jobs, id: :string do |t|
  t.string :project_id
  t.string :api_job_id

  t.string :kind, null: false
  t.string :state, null: false, default: "queued"
  t.string :stage

  t.integer :processed, null: false, default: 0
  t.integer :total, null: false, default: 0
  t.integer :failed, null: false, default: 0

  t.string :mode
  t.string :status_url
  t.string :result_url
  t.string :candidate_set_id

  t.jsonb :input, null: false, default: {}
  t.jsonb :result, null: false, default: {}
  t.jsonb :error, null: false, default: {}

  t.datetime :started_at
  t.datetime :finished_at
  t.timestamps
end

add_index :engine_jobs, :project_id
add_index :engine_jobs, :api_job_id, unique: true, where: "api_job_id IS NOT NULL"
add_index :engine_jobs, [:project_id, :kind]
add_index :engine_jobs, :state
```

### Kinds

```text
mapper_drugs_batch
hybrid_search_batch
import_source_file
create_export
catalog_build
tantivy_build
sapbert_build
thirawat_embed_build
tachiom_build
```

### States

```text
queued
starting
running
succeeded
succeeded_with_errors
failed
cancelled
```

### Rules

1. Rails mirrors engine jobs for UI and workflow.
2. Rails does not read engine `jobs.sqlite`.
3. `api_job_id` links to API jobs when present.
4. Hybrid search batch jobs may be Rails-side only and not have `api_job_id`.
5. Job updates should broadcast Turbo replacements.

---

## 11. `exports`

### Migration

```ruby
create_table :exports, id: :string do |t|
  t.string :project_id, null: false
  t.string :created_by_id, null: false

  t.string :format, null: false
  t.string :state, null: false, default: "queued"
  t.string :file_name
  t.string :content_type

  t.jsonb :params, null: false, default: {}
  t.jsonb :summary, null: false, default: {}
  t.jsonb :error, null: false, default: {}

  t.datetime :started_at
  t.datetime :finished_at
  t.timestamps
end

add_index :exports, :project_id
add_index :exports, :state
```

### Attachment

```ruby
has_one_attached :file
```

### Formats

```text
usagi_csv
source_to_concept_map
review_csv
candidate_jsonl
candidate_csv
audit_csv
```

---

## 12. `audit_events`

### Migration

```ruby
create_table :audit_events, id: :string do |t|
  t.string :project_id, null: false
  t.string :mapping_id
  t.string :actor_id

  t.string :event_type, null: false
  t.jsonb :before, null: false, default: {}
  t.jsonb :after, null: false, default: {}
  t.jsonb :metadata, null: false, default: {}

  t.timestamps
end

add_index :audit_events, :project_id
add_index :audit_events, :mapping_id
add_index :audit_events, :event_type
add_index :audit_events, :created_at
```

### Event types

```text
project_created
source_terms_imported
mapping_candidate_search_completed
mapping_candidate_applied
mapping_status_changed
mapping_bulk_status_changed
comment_created
export_created
engine_job_started
engine_job_failed
engine_job_succeeded
```

### Rule

Audit events are append-only.

---

## 13. `comments`

### Migration

```ruby
create_table :comments, id: :string do |t|
  t.string :project_id, null: false
  t.string :mapping_id, null: false
  t.string :author_id, null: false
  t.text :body, null: false
  t.timestamps
end

add_index :comments, :project_id
add_index :comments, :mapping_id
add_index :comments, :author_id
```

### Rules

```text
comments belong to mapping rows
comments write audit_events
comments are visible in candidate drawer
```

---

## 14. Query patterns and indexes

### Mapping review index

Typical filters:

```text
project_id
mapping_status
source text query
target concept query
target_domain_id
target_vocabulary_id
has candidates
has target
```

Indexes needed:

```text
mappings(project_id, mapping_status)
mappings(project_id, target_domain_id)
mappings(project_id, target_vocabulary_id)
source_terms(project_id, source_code)
mapping_candidates(mapping_id, rank)
```

For source/target text search, start with `ILIKE` and indexes only where needed. Add PostgreSQL trigram indexes if real usage demands it. Do not summon Elasticsearch for 5,000 rows unless you enjoy unnecessary infrastructure pets.

---

## 15. Batch persistence

Use batch inserts for:

```text
source_terms
mappings
mapping_candidates
```

For candidate persistence:

```text
insert_all / upsert_all
batch size 1,000-5,000 rows
unique_by index on mapping_id/concept_id/method
```

Complexity:

```text
source import: O(N)
candidate persistence: O(P * K)
status update selected rows: O(S)
```

Where:

```text
N = source rows
P = source terms
K = candidates per source term
S = selected mappings
```

---

## 16. Lifecycle constraints

### Import lifecycle

```text
pending -> previewed -> queued -> running -> succeeded
                              -> failed
                              -> cancelled
```

### Mapping lifecycle

```text
source term imported
  -> mapping created UNCHECKED
  -> candidates added manually or by job
  -> candidate optionally applied
  -> reviewer approves/flags/invalidates
  -> export includes approved mappings by default
```

### Engine job lifecycle

```text
queued/starting -> running -> succeeded
                         -> succeeded_with_errors
                         -> failed
                         -> cancelled
```

---

## 17. Data deletion and archiving

MVP behavior:

```text
archive projects instead of hard delete
keep source terms/mappings/candidates/audit
allow export records to remain even if file is purged later
```

Hard delete can be admin-only later.

---

## 18. Privacy and PHI stance

Source terms may contain sensitive local labels depending on the dataset.

Rules:

```text
do not log raw source file contents
do not log full source rows
do not expose source files outside project permission checks
do not send source terms to any external third-party API
engine services are internal only
```

`usagi-api` is internal infrastructure, not a third-party SaaS escape hatch.
