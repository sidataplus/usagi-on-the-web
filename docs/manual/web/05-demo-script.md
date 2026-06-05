# Local Docker Demo Script

Status: draft v0.1
Audience: demo presenter, reviewer, local operator

## Goal

Demonstrate the complete local mapping loop without sign-in:

```text
Augmentin 875/125
  -> THIRAWAT/Tachiom candidates
  -> amoxicillin 875 MG / clavulanate 125 MG Oral Tablet
  -> human approval
  -> export readiness
```

The demo runs on local Docker Compose. Rails owns the browser workflow, while
the Rust engine services provide catalog, hybrid search, and drug mapping over
signed private HTTP.

## Start

From the repository root:

```bash
docker compose \
  -f infra/docker/docker-compose.yml \
  -f infra/docker/docker-compose.debug.yml \
  up -d --build
```

Then prove the stack:

```bash
USAGI_DEPLOY_SMOKE_START=0 \
USAGI_DEPLOY_SMOKE_DEBUG_PORTS=1 \
USAGI_DEPLOY_SMOKE_RUN_LIVE_TESTS=1 \
scripts/smoke-deploy.sh
```

Expected smoke highlights:

```text
catalog: ready
search: ready
mapper: ready
hybrid_search_top_concept_id: 123456
mapper_top_concept_id: 123456
mapper_top_concept_name: amoxicillin 875 MG / clavulanate 125 MG Oral Tablet
Rails live tests: 0 failures
```

## Browser Walkthrough

Open:

```text
http://127.0.0.1:3000/projects
```

Expected first view:

```text
Local Reviewer
Demo: Augmentin Drug Review
0 of 1 approved
```

Open the demo project, then the review tab or direct URL:

```text
/projects/<project_id>/mappings?status=unchecked
```

Expected review view:

```text
Augmentin 875/125
SRC_AUGMENTIN_875_125
3 saved candidates
Best candidate:
amoxicillin 875 MG / clavulanate 125 MG Oral Tablet
Unchecked 1
Approved 0
```

Click `Open next unchecked`.

Expected review cockpit:

```text
Target: no target yet
Approve: disabled
Suggested candidates: 3 saved candidates
Rank 1: amoxicillin 875 MG / clavulanate 125 MG Oral Tablet
Concept id: 123456
Method: thirawat_tachiom_bimaxsim_tiebreak
Provenance model: sidataplus/THIRAWAT-SapBERT
Provenance index: /data/mapper/thirawat-drug/tachiom/manifest.json
```

Click `Use` on rank 1.

Expected selected state:

```text
Target: amoxicillin 875 MG / clavulanate 125 MG Oral Tablet
Approve: enabled
```

Click `Approve`.

Expected review refresh:

```text
All mappings reviewed
Unchecked 0
Approved 1
No mappings match these filters.
```

Open `Exports`.

Expected export readiness:

```text
1 approved mapping
0 unchecked mappings still need review
Review CSV
USAGI CSV
SOURCE_TO_CONCEPT_MAP CSV
Candidate provenance (CSV)
Candidate provenance (JSONL)
Audit trail CSV
```

## Reset For Another Live Demo

If the row has already been approved, reset only the demo mapping:

```bash
docker compose \
  -f infra/docker/docker-compose.yml \
  -f infra/docker/docker-compose.debug.yml \
  exec -T rails-web bin/rails runner '
    project = Project.find_by!(name: "Demo: Augmentin Drug Review")
    mapping = project.mappings.joins(:source_term)
                     .find_by!(source_terms: { source_code: "SRC_AUGMENTIN_875_125" })
    mapping.mapping_candidates.update_all(selected: false)
    mapping.update!(
      mapping_status: "UNCHECKED",
      target_concept_id: nil,
      target_concept_name: nil,
      target_domain_id: nil,
      target_vocabulary_id: nil,
      target_concept_class_id: nil,
      target_standard_concept: nil,
      target_concept_code: nil,
      match_score: nil,
      search_method: nil,
      reviewed_by: nil,
      reviewed_at: nil
    )
    puts({ status: mapping.reload.mapping_status,
           unchecked: project.mappings.unchecked.count,
           approved: project.mappings.approved.count }.to_json)
  '
```

Expected reset output:

```json
{"status":"UNCHECKED","unchecked":1,"approved":0}
```

## Narrow Width Check

The demo is desktop-oriented, but it should remain usable at narrow widths.
At roughly `390px` wide:

```text
no horizontal page overflow
review table collapses into card-like rows
review cockpit fits without clipping candidate actions
post-approval counts refresh without stale unchecked state
exports readiness stays readable
```
