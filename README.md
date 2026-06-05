# Usagi-on-the-Web v3

Usagi-on-the-Web v3 is an OMOP concept mapping workbench.

```text
Browser
  -> Rails 8 app in apps/web
  -> private signed usagi-api services
```

Rails owns product workflow state: users, projects, imports, source terms,
mappings, candidates, comments, audit events, exports, and engine job mirrors.
The Rust `usagi-api` services own catalog, search, mapper, job, model, and
artifact internals.

## Architecture

### Product Runtime

```text
+---------+
| Browser |
+----+----+
     | HTTPS
     v
+------------------------------+
| Rails 8 app                  |
| apps/web                     |
|                              |
| owns product workflow state  |
| users, projects, imports     |
| mappings, reviews, exports   |
+----+-------------------------+
     | signed private HTTP
     v
+------------------------------+
| usagi-api services           |
|                              |
| catalog-api  search-api      |
| mapper-api   api-worker      |
|                              |
| owns engine internals        |
| catalog, indexes, models     |
| jobs, artifact results       |
+------------------------------+
```

The browser never calls engine services directly in production. Rails talks to
engines only through signed service clients.

### Rails Web Responsibilities

```text
+--------------------------------------------------------------+
| rails-web                                                    |
| apps/web                                                     |
|                                                              |
|  HTTP pages + Turbo flows                                    |
|     |                                                        |
|     +--> authentication                                      |
|     +--> project membership + authorization                  |
|     +--> imports and source terms                            |
|     +--> mapping review workspace                            |
|     +--> manual search + auto-suggest orchestration          |
|     +--> comments, audit events, exports                     |
|     +--> engine job mirrors and user-safe error states       |
|                                                              |
|  product database                                            |
|     |                                                        |
|     +--> users, projects, mappings, candidates               |
|     +--> import sessions, comments, audit events             |
|     +--> exports, engine_jobs mirrors                        |
|                                                              |
|  signed engine clients                                       |
|     |                                                        |
|     +--> catalog-api for concept metadata                    |
|     +--> search-api for non-drug/hybrid candidates           |
|     +--> mapper-api for drug candidates and job polling      |
+--------------------------------------------------------------+

Rails rule:
  keep product workflow state here
  do not read engine artifact files
  do not let the browser call engine APIs directly
```

### API Service Responsibilities

```text
+------------------------------+       +------------------------------+
| catalog-api                  |       | search-api                   |
|                              |       |                              |
| runtime truth:               |       | candidate search:            |
|  /data/catalog/catalog.sqlite|       |  Tantivy lexical             |
|                              |       |  SapBERT CLS + USearch       |
| endpoints:                   |       |  hybrid RRF                  |
|  concept lookup              |       |                              |
|  batch lookup                |       | endpoints:                   |
|  relationships               |       |  /search/concepts            |
|  domains/vocabs/classes      |       |  /search/batch               |
|  catalog build job           |       |  /search/explain             |
|                              |       |  Tantivy/SapBERT build jobs  |
| inputs/artifacts:            |       |                              |
|  Athena CSVs -> SQLite       |       | inputs/artifacts:            |
+------------------------------+       |  catalog.sqlite              |
                                       |  search/tantivy/index        |
                                       |  search/sapbert/*.usearch    |
                                       +------------------------------+

+------------------------------+       +------------------------------+
| mapper-api                   |       | api-worker                   |
|                              |       |                              |
| drug mapping engine:         |       | background artifact/job work: |
|  THIRAWAT-SapBERT            |       |  catalog builds              |
|  Tachiom retrieval           |       |  Tantivy builds              |
|  exact BiMaxSim rerank       |       |  SapBERT embedding/index     |
|  deterministic tie-breaker   |       |  THIRAWAT doc embeddings     |
|                              |       |  Tachiom builds              |
| endpoints:                   |       |  mapper batch jobs           |
|  /mapper/drugs/query         |       |                              |
|  /mapper/drugs/batch         |       | queues:                      |
|  /mapper/drugs/batch-job     |       |  catalog, index, embed, map  |
|  /jobs/<id>                  |       |                              |
|  mapper build-job routes     |       | owns:                        |
|                              |       |  jobs.sqlite                 |
| inputs/artifacts:            |       |  jobs/results/               |
|  thirawat model              |       |  temp build directories      |
|  doc_embeddings              |       +------------------------------+
|  tachiom index               |
+------------------------------+
```

### Request And Job Flow

```text
Manual review search:

Reviewer
  -> rails-web mapping page
      -> signed request to search-api or mapper-api
          -> candidate DTOs
      -> rails-web persists MappingCandidate rows
      -> reviewer explicitly accepts/rejects mapping

Auto-suggest batch:

Reviewer
  -> rails-web starts auto-map
      -> signed mapper-api/search-api job request
          -> api-worker processes queued engine job
          -> job results written under /data/jobs/results
      -> rails-web polls job endpoint
      -> rails-web mirrors status/results into engine_jobs
      -> reviewer still owns final mapping decisions
```

### Local Compose

```text
Host
  |
  | http://127.0.0.1:3000
  v
+--------------------------------------------------------------------------------------+
| Docker Compose                                                                       |
|                                                                                      |
|  public network                                                                      |
|    +-- rails-web                                                                     |
|          |                                                                           |
|          | app_private network                                                       |
|          v                                                                           |
|    +----------+     +-------------+     +------------+     +------------+            |
|    | postgres |     | catalog-api |     | search-api |     | mapper-api |            |
|    +----------+     +-------------+     +------------+     +-----+------+            |
|                                                                  |                   |
|                                                                  v                   |
|                                                             +------------+           |
|                                                             | api-worker |           |
|                                                             +------------+           |
+--------------------------------------------------------------------------------------+

Debug override only:
  127.0.0.1:28788 -> catalog-api
  127.0.0.1:28789 -> search-api
  127.0.0.1:28790 -> mapper-api
```

### Deployment Shapes

```text
Option A: Single cloud host

Browser
  -> Kamal Rails host
       -> Postgres
       -> catalog/search/mapper/api-worker accessories
       -> /data artifact volume

Option B: Split cloud hosts

Browser
  -> Kamal Rails host
       -> managed Postgres (DO or AWS RDS)
       -> private engine host
            -> catalog/search/mapper/api-worker
            -> /data artifact volume

Option C: Local API compute over Tailscale

Browser
  -> cloud Rails host (DigitalOcean or AWS EC2)
       -> managed Postgres
       -> Tailscale private network
            -> local API compute server
                 -> catalog/search/mapper/api-worker
                 -> /data artifact volume
```

### Artifact Build And Distribution

```text
Athena vocabulary files
  -> catalog.sqlite
      -> Tantivy lexical index
      -> SapBERT CLS + USearch index
      -> THIRAWAT Drug document embeddings
          -> Tachiom index

Optional remote stage:

catalog.sqlite + THIRAWAT model
  -> Modal build pipeline
      -> mapper/thirawat-drug/doc_embeddings/
      -> artifact pack (.tar.zst)
      -> private object storage or Hugging Face Dataset
      -> restore into API /data volume
      -> build Tachiom index
```

## Repo Layout

```text
apps/web/          Rails product workflow layer
services/          Rust HTTP services
crates/            Shared Rust crates
infra/docker/      Local Docker Compose deployment
infra/kamal/       Kamal/DigitalOcean operator notes
infra/aws/         AWS EC2 + RDS operator notes
infra/modal/       Modal artifact build pipeline
docs/              Specs, manuals, security, and testing docs
fixtures/          Tiny development fixtures
scripts/           Smoke and verification scripts
```

## Local Rails

```bash
cd apps/web
bin/setup
bin/rails test
bin/rails server
```

Rails uses stubbed engine clients by default for local development and tests.

## Local Full-Stack Deployment

```bash
docker compose -f infra/docker/docker-compose.yml up -d --build
scripts/smoke-deploy.sh
```

Only Rails publishes a host port by default. To expose engine ports for local
debugging or host-side live tests:

```bash
docker compose \
  -f infra/docker/docker-compose.yml \
  -f infra/docker/docker-compose.debug.yml \
  up -d --build
```

The debug override publishes `127.0.0.1:28788-28790` by default so host tests
do not clash with native `usagi-api` dev servers on `8788-8790`.

Production can run on **DigitalOcean** or **AWS (EC2 + RDS)** while keeping
compute-heavy API services on a separate host or Tailscale. See
`docs/manual/web/04-deployment.md` for deploy steps and sizing guides, and
`infra/aws/README.md` for the RDS cutover checklist.

Container images are published by GitHub Actions to:

```text
ghcr.io/sidataplus/usagi-on-the-web/rails-web
ghcr.io/sidataplus/usagi-on-the-web/catalog-api
ghcr.io/sidataplus/usagi-on-the-web/search-api
ghcr.io/sidataplus/usagi-on-the-web/mapper-api
ghcr.io/sidataplus/usagi-on-the-web/api-worker
```

## Verification

Rails:

```bash
cd apps/web
bin/rails test
bin/rubocop
bin/brakeman --no-pager
```

Rust:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Deployment smoke:

```bash
scripts/smoke-signed-auth.sh
scripts/smoke-deploy.sh
USAGI_DEPLOY_SMOKE_START=0 USAGI_DEPLOY_SMOKE_DEBUG_PORTS=1 USAGI_DEPLOY_SMOKE_RUN_LIVE_TESTS=1 scripts/smoke-deploy.sh
```

The last command runs gated Rails live engine and workflow E2E tests against the
signed compose stack.

## Docs

Start with:

```text
AGENTS.md
docs/README.md
docs/manual/web/04-deployment.md
docs/manual/api/00-deployment.md
docs/manual/api/05-artifact-builds.md
docs/security/rails-api-boundary.md
infra/modal/README.md
```

Do not commit runtime databases, indexes, model weights, or generated artifacts.
