# Rails Deployment Manual

Status: draft v0.2  
Audience: Rails operators and deploy reviewers

## Purpose

This manual describes the first deployment-ready shape for Usagi-on-the-Web v3:

```text
Browser
  -> Rails 8 app
  -> private signed usagi-api services
```

Rails owns product workflow state. The engine services own catalog, search,
mapper, job, and artifact internals. Production and local deployment must not
allow the browser to call engine services directly.

## Local Full-Stack Compose

The default full-stack Compose file starts:

```text
rails-web
postgres
catalog-api
search-api
mapper-api
api-worker
```

Only Rails publishes a host port by default. Engine services are reachable on
the private Docker network from Rails only.

Start the stack:

```bash
docker compose \
  --env-file infra/docker/.env.example \
  -f infra/docker/docker-compose.yml \
  up -d --build
```

Then smoke it:

```bash
scripts/smoke-deploy.sh
```

The smoke script checks:

```text
Rails /up from the host
engine health from inside the rails-web container
Rails engine clients over signed HTTP
```

Local compose enables `USAGI_LOCAL_DEPLOY=1` by default. Rails auto-signs in a
single passwordless reviewer account (`local@usagi.test`) and seeds one demo
project on first boot. Do not enable this flag on internet-facing production
hosts.

Stop the stack:

```bash
docker compose -f infra/docker/docker-compose.yml down
```

### Debug Engine Ports

Use the debug override only when a developer needs host access to engine ports
for host-side live Rails tests or manual API debugging:

```bash
docker compose \
  -f infra/docker/docker-compose.yml \
  -f infra/docker/docker-compose.debug.yml \
  up -d --build
```

By default this publishes the compose stack on non-conflicting host ports:

```text
127.0.0.1:28788 -> catalog-api
127.0.0.1:28789 -> search-api
127.0.0.1:28790 -> mapper-api
```

The defaults avoid clashing with native `usagi-api` dev servers that usually
bind `8788-8790`. Override with `CATALOG_PUBLISH_PORT`, `SEARCH_PUBLISH_PORT`,
and `MAPPER_PUBLISH_PORT` when needed.

The debug override also attaches engine services to the public compose network
so host port publishing works. Engine services stay on the internal
`app_private` network in the base compose file.

Do not use the debug override for production.

## Production Kamal Shape

Kamal deploys Rails from `apps/web/config/deploy.yml`. The template assumes:

```text
GitHub Container Registry
one Rails web role
PostgreSQL accessory
engine service accessories
signed Rails-to-engine requests
Kamal proxy with TLS
```

Before deploy, export operator-specific values:

```bash
export KAMAL_WEB_HOST=203.0.113.10
export KAMAL_ENGINE_HOST=203.0.113.10
export KAMAL_DB_HOST=203.0.113.10
export APP_HOST=usagi.example.org
export APP_HOSTS=usagi.example.org
export GHCR_USERNAME=sidataplus
export GHCR_IMAGE_PREFIX=sidataplus/usagi-on-the-web
export USAGI_IMAGE_TAG=latest
```

Export secrets from a password manager or CI secret store:

```bash
export GHCR_TOKEN=...
export RAILS_MASTER_KEY=...
export SECRET_KEY_BASE=...
export POSTGRES_PASSWORD=...
export DATABASE_URL=postgres://usagi:${POSTGRES_PASSWORD}@db:5432/usagi_production
export USAGI_API_SHARED_SECRET=...
```

Validate and deploy:

```bash
cd apps/web
bin/kamal config
bin/kamal setup
bin/kamal deploy
```

The engine accessory images referenced by `config/deploy.yml` must already be
available in the registry:

```text
ghcr.io/sidataplus/usagi-on-the-web/catalog-api:latest
ghcr.io/sidataplus/usagi-on-the-web/search-api:latest
ghcr.io/sidataplus/usagi-on-the-web/mapper-api:latest
ghcr.io/sidataplus/usagi-on-the-web/api-worker:latest
```

The `Container Images` GitHub Actions workflow publishes these images to GHCR
on `main` and by manual dispatch. Kamal pushes the Rails image to
`ghcr.io/sidataplus/usagi-on-the-web/rails-web` using `GHCR_TOKEN`.

## DigitalOcean Droplet Sizing

These are planning estimates, not benchmark results. They assume:

```text
full Athena standard-concept artifacts (not fixture-mini builds)
CPU inference in Rust/Candle (no GPU in the current Kamal template)
a small mapping team (roughly 5-20 reviewers), not public SaaS traffic
SOLID_QUEUE_IN_PUMA=true on the Rails host
```

DigitalOcean list prices change; treat the monthly figures as order-of-magnitude
guides for Basic/Premium droplets plus block storage.

### Rails Only On DigitalOcean

Use this when engine services run elsewhere, such as **Production Option 3**
(Tailscale/local API compute). The droplet runs Kamal proxy, Rails, and ideally
not Postgres.

| Tier | Shape | Storage | Monthly guide | Good for |
|---|---|---|---|---|
| Pilot | 2 vCPU / 4 GB | 25 GB | ~$24 | single project, few users |
| Recommended | 2 vCPU / 4 GB + **Managed Postgres** (1-2 GB) | 25-50 GB Rails volume | ~$24 + ~$15-30 DB | small team production |
| Comfortable | 4 vCPU / 8 GB + Managed Postgres (2-4 GB) | 50 GB | ~$48 + ~$30-60 DB | heavier imports, exports, queue backlog |

Rough RAM budget on the Rails droplet:

| Component | RAM |
|---|---|
| OS + Docker/Kamal | 0.5-1 GB |
| Rails/Puma (1-2 workers) | 0.5-1.5 GB |
| Solid Queue in Puma | 0.25-0.5 GB |
| co-located Postgres accessory | 1-2 GB extra |
| headroom | ~1 GB |

Prefer **Managed PostgreSQL** and keep the web droplet at **4 GB / 2 vCPU**.
Co-locating Postgres on the same 4 GB host works for a pilot but gets tight once
imports, exports, and background jobs overlap.

Engine `/data` does not live on this droplet.

### Rails + API On One DigitalOcean Droplet

Use this when `config/deploy.yml` deploys Rails, Postgres, and all engine
accessories to the same `KAMAL_WEB_HOST` / `KAMAL_ENGINE_HOST`.

| Tier | Shape | Block storage for `/data` | Monthly guide | Good for |
|---|---|---|---|---|
| Minimum viable | 8 vCPU / 16 GB | 100 GB | ~$96 + ~$10 | demo, very light concurrent use |
| Recommended | 8 vCPU / 32 GB | 160 GB | ~$192 + ~$16 | small production team |
| Split alternative | see below | separate volumes | ~$120-220 total | better isolation |

Rough runtime RAM by service on the engine host:

| Service | Role | RAM at runtime |
|---|---|---|
| `catalog-api` | SQLite catalog lookups | 0.5-1 GB |
| `search-api` | Tantivy + SapBERT hybrid search | 2-4 GB |
| `mapper-api` | THIRAWAT + Tachiom + BiMaxSim | 4-8 GB |
| `api-worker` | embed/map/index jobs | 2-6 GB when busy |
| Rails + Postgres | product workflow | 2-4 GB |
| OS + Docker overhead | | 1-2 GB |

Mapper batch jobs and hybrid search are CPU-heavy. On 16 GB RAM you will often
hit CPU limits before memory limits if several auto-map jobs overlap. The API
worker defaults assume low map concurrency (1-2) and one embed worker per model.

### Split Rails And Engine Droplets On DigitalOcean

This is the preferred all-DO shape once you leave pilot mode:

| Role | Shape | Storage | Monthly guide |
|---|---|---|---|
| Web | 2 vCPU / 4 GB + Managed Postgres | 25-50 GB Rails storage | ~$24 + DB |
| Engine | 8 vCPU / 32 GB | 160 GB `/data` volume | ~$192 + ~$16 |

Set `KAMAL_WEB_HOST` and `KAMAL_DB_HOST` on the web droplet. Set
`KAMAL_ENGINE_HOST` on the engine droplet. Keep engine ports off the public
internet.

### API Compute Server Sizing (Tailscale Or On-Prem)

When APIs run off DigitalOcean, size the compute host like the engine column
above:

| Tier | Shape | `/data` storage | Notes |
|---|---|---|---|
| Minimum | 8 vCPU / 16 GB | 100 GB | serving only; avoid rebuilds during peak use |
| Recommended | 8 vCPU / 32 GB | 160 GB | serving + occasional artifact rebuilds |
| Build maintenance window | 16+ vCPU / 64+ GB | 160+ GB | faster Athena rebuilds; can be a separate temporary host |

Publish API ports only on the Tailscale interface (`USAGI_PUBLISH_HOST=100.x.y.z`
or a `.ts.net` MagicDNS name). Firewall public ingress.

### `/data` Disk Planning

For a full Athena standard-concept build:

| Artifact area | Order of magnitude |
|---|---|
| `catalog.sqlite` | 1-3 GB |
| Tantivy index | 1-3 GB |
| SapBERT USearch + sidecars | 3-6 GB |
| SapBERT + THIRAWAT model weights | 1-2 GB |
| THIRAWAT drug embeddings + Tachiom | 5-20 GB |
| jobs DB + results | grows with use |
| **Total provisioned** | **30-80 GB**; budget **100-160 GB** with rebuild headroom |

### When To Scale Up

```text
several concurrent drug auto-map batches -> engine host 32 GB / 8+ vCPU
full-vocabulary rebuild on the serving host -> temporary larger build host
large imports/exports or Solid Queue backlog -> Rails 8 GB or Managed Postgres upgrade
```

## AWS With RDS PostgreSQL

Kamal on EC2 works with the same `apps/web/config/deploy.yml` template. The main
difference from DigitalOcean is that **product Postgres moves to RDS** (or Aurora
PostgreSQL) instead of the Kamal `db` accessory.

Detailed operator notes live in `infra/aws/README.md`.

### What Stays The Same

```text
Kamal deploy of rails-web from GHCR
signed Rails-to-engine requests
engine accessories on EC2 (or Tailscale/off-AWS compute)
Solid Queue/Cache/Cable SQLite on the Rails EBS volume
Active Storage on local disk unless you later move uploads to S3
deployment_checks and smoke scripts
```

Rails already uses `DATABASE_URL`; no application code change is required for
RDS as long as the URL is a valid Postgres connection string.

### What Changes

| Area | Kamal + DO accessory | Kamal + AWS RDS |
|---|---|---|
| Product database | `accessories.db` container | **RDS PostgreSQL** |
| `DATABASE_URL` | `postgres://usagi:…@db:5432/…` | `postgres://…@….rds.amazonaws.com:5432/…?sslmode=require` |
| `POSTGRES_PASSWORD` | Kamal secret for accessory | RDS master password or Secrets Manager |
| Rails host | Droplet | EC2 |
| Engine `/data` | DO block volume | EBS gp3 on engine EC2 |
| Public ingress | Kamal proxy + Let's Encrypt | Kamal proxy on EC2 or ALB + ACM |
| `KAMAL_DB_HOST` | set | **omit**; disable `db` accessory |

Before deploy with RDS:

```text
1. Remove or comment out the accessories.db block in deploy.yml
2. Stop exporting POSTGRES_PASSWORD for Kamal unless another tool needs it
3. Export DATABASE_URL with the RDS endpoint and sslmode=require
4. Run bin/rails db:prepare (or restore a dump) against RDS once
```

Example:

```bash
export DATABASE_URL="postgres://usagi:${DB_PASSWORD}@usagi-prod.abc123.us-east-1.rds.amazonaws.com:5432/usagi_production?sslmode=require"
```

`DeploymentChecks` validates engine URLs for private hosts only. An
`*.rds.amazonaws.com` hostname in `DATABASE_URL` is expected and is not
checked by those rules.

### RDS vs Aurora

For a small internal mapping team, prefer **RDS PostgreSQL** first:

| Service | Use when |
|---|---|
| **RDS PostgreSQL** | default v1 production; simpler and usually cheaper |
| **Aurora PostgreSQL (provisioned)** | you need faster failover or read replicas soon |
| **Aurora Serverless v2** | highly spiky load; usually unnecessary for this product |

Practical RDS starting point:

```text
Engine: PostgreSQL 16 or 17
Instance: db.t4g.medium (2 vCPU, 4 GB) or db.t4g.large
Storage: 20-50 GB gp3 with autoscaling
Network: private subnets only; no public RDS endpoint
Backups: automated snapshots + PITR enabled
Multi-AZ: optional HA; doubles DB cost
```

### Recommended AWS Topologies

**Rails on EC2 + RDS + API over Tailscale** (cheapest AWS product path):

```text
Internet -> EC2 (Kamal Rails + proxy)
              -> RDS PostgreSQL (private subnet)
              -> Tailscale -> API compute (on-prem or separate host)
```

Disable all engine accessories in `deploy.yml` for that environment. Set engine
URLs to Tailscale hosts as in **Production Option 3**.

**Split on AWS** (preferred if engines stay in AWS):

```text
EC2 web   -> Kamal Rails only
RDS       -> product database
EC2 engine -> catalog/search/mapper/api-worker + EBS /data
```

Set `KAMAL_WEB_HOST` on the web instance and `KAMAL_ENGINE_HOST` on the engine
instance. Point engine URLs at the engine instance **private IP** or an internal
DNS name reachable from the web security group.

### AWS Instance Sizing

Translate from the DigitalOcean guide:

| Role | AWS instance | Storage |
|---|---|---|
| Rails only | **t3.medium** (2 vCPU, 4 GB) or **t3.large** | 25-50 GB EBS |
| Rails + API on one host | **r6i.2xlarge** (8 vCPU, 64 GB) | 160 GB EBS for `/data` |
| Engine only | **r6i.2xlarge** or **m6i.2xlarge** (8 vCPU, 32 GB) | 160 GB EBS gp3 |
| RDS | **db.t4g.medium** -> **db.t4g.large** | 20-50 GB gp3 |

The current Kamal image builder targets **amd64**. Use **t3/m6i/r6i** unless you
add arm64 image builds for Graviton.

Engine `/data` disk planning matches the table in **DigitalOcean Droplet Sizing**
above.

### Security Groups (Minimum)

| Source | Destination | Ports |
|---|---|---|
| Internet or ALB | Web EC2 | 80, 443 |
| Web EC2 SG | RDS SG | 5432 |
| Web EC2 SG | Engine EC2 SG | 8788, 8789, 8790 |
| Operator IP (optional) | Web EC2 | 22 |

Do not expose engine ports to `0.0.0.0/0`.

### Backups On AWS

| Asset | Where | Backup |
|---|---|---|
| Product DB | RDS | automated snapshots + PITR |
| Uploads / exports | Rails EBS | EBS snapshots |
| Solid Queue/Cache/Cable SQLite | Rails EBS | same snapshot as Rails volume |
| Engine `/data` | Engine EBS | EBS snapshots |

A later improvement is Active Storage on **S3** so the Rails EC2 instance is
more disposable. That is not required for the first AWS cutover.

### Kamal Env Example (RDS)

```bash
export KAMAL_WEB_HOST=203.0.113.10
export KAMAL_ENGINE_HOST=203.0.113.20
export APP_HOST=usagi.example.org
export APP_HOSTS=usagi.example.org
export GHCR_USERNAME=sidataplus
export GHCR_IMAGE_PREFIX=sidataplus/usagi-on-the-web
export USAGI_IMAGE_TAG=latest

export DATABASE_URL="postgres://usagi:${DB_PASSWORD}@usagi-prod.abc123.us-east-1.rds.amazonaws.com:5432/usagi_production?sslmode=require"
export USAGI_API_SHARED_SECRET=...

# Same-VPC engine private IP example:
export CATALOG_API_URL=http://10.0.2.20:8788
export SEARCH_API_URL=http://10.0.2.20:8789
export MAPPER_API_URL=http://10.0.2.20:8790
export JOBS_API_URL=http://10.0.2.20:8790
```

Registry can stay on **GHCR** or mirror images to **ECR** for faster pulls inside
AWS.

### Migration Checklist (DO -> AWS)

```text
1. Provision VPC, subnets, RDS, EC2 instance(s), security groups, EBS volumes
2. Restore Postgres to RDS or run bin/rails db:prepare against an empty RDS
3. Copy Rails storage and engine /data to EBS
4. Disable accessories.db in deploy.yml
5. Export DATABASE_URL, engine URLs, and Kamal secrets
6. bin/kamal setup && bin/kamal deploy
7. scripts/smoke-signed-auth.sh and production smoke against APP_HOST
```

## Production Option 3: Local API Compute Over Tailscale

Use this topology when `catalog-api`, `search-api`, `mapper-api`, and
`api-worker` need local GPU/CPU/RAM resources that are larger or cheaper than
the DigitalOcean Rails host.

```text
Browser
  -> cloud Rails/Kamal host (DigitalOcean or AWS EC2 + RDS)
      -> Tailscale private network
          -> local API compute server
```

Rails and PostgreSQL stay on the cloud web host. The compute-heavy API stack
runs on the local server with the same `/data` artifact layout. Rails still
calls only the engine HTTP APIs, and the browser still never calls engine
services.

### Network Setup

1. Install Tailscale on the Rails host (droplet or EC2).
2. Install Tailscale on the local API compute server.
3. Put both machines in the same tailnet.
4. Use MagicDNS names or stable Tailscale IPs for API URLs.
5. Firewall engine ports so they are reachable only from the tailnet.

Example Tailscale URLs:

```text
CATALOG_API_URL=http://usagi-api.my-tailnet.ts.net:8788
SEARCH_API_URL=http://usagi-api.my-tailnet.ts.net:8789
MAPPER_API_URL=http://usagi-api.my-tailnet.ts.net:8790
JOBS_API_URL=http://usagi-api.my-tailnet.ts.net:8790
```

If MagicDNS is not enabled, use the local server's Tailscale `100.x.y.z`
address. Rails production boot accepts Tailscale MagicDNS names ending in
`.ts.net` and engine URLs whose host is an IP in `100.64.0.0/10`.

### Local API Compute Server

On the local server, run the API-only Compose stack and publish ports on the
Tailscale interface:

```bash
export USAGI_PUBLISH_HOST=100.x.y.z
export USAGI_API_ENV=production
export USAGI_API_AUTH_MODE=signed
export USAGI_API_SHARED_SECRET=...

docker compose -f infra/docker/docker-compose.api.yml up -d --build
```

The local server must keep `/data` on durable local storage and should be backed
up independently from the DigitalOcean Rails host. Build catalog/search/mapper
artifacts on this machine using `docs/manual/api/05-artifact-builds.md`.

Size the API compute host using **API Compute Server Sizing** above. A practical
starting point is **8 vCPU / 32 GB RAM** with a **160 GB** `/data` volume.

### Cloud Rails Env

Before `bin/kamal deploy`, export the Tailscale URLs:

```bash
export CATALOG_API_URL=http://usagi-api.my-tailnet.ts.net:8788
export SEARCH_API_URL=http://usagi-api.my-tailnet.ts.net:8789
export MAPPER_API_URL=http://usagi-api.my-tailnet.ts.net:8790
export JOBS_API_URL=http://usagi-api.my-tailnet.ts.net:8790
export USAGI_API_SHARED_SECRET=...
```

For this option, do not deploy the engine accessories from `config/deploy.yml`.
Keep the PostgreSQL accessory or use managed PostgreSQL, but remove or disable
the `catalog-api`, `search-api`, `mapper-api`, and `api-worker` accessories for
that deployment environment.

### Verification

From the Rails host:

```bash
curl -fsS http://usagi-api.my-tailnet.ts.net:8788/catalog/health
curl -fsS http://usagi-api.my-tailnet.ts.net:8789/search/health
curl -fsS http://usagi-api.my-tailnet.ts.net:8790/mapper/health
```

From Rails:

```bash
cd apps/web
bin/kamal app exec "bin/rails runner 'puts EngineClients::SearchClient.new.status.inspect'"
```

Then run the live workflow proof from a machine that can reach the Tailscale API
URLs:

```bash
cd apps/web
USAGI_LIVE_ENGINE=1 \
ENGINE_CLIENT_MODE=http \
USAGI_API_SHARED_SECRET=... \
CATALOG_API_URL=http://usagi-api.my-tailnet.ts.net:8788 \
SEARCH_API_URL=http://usagi-api.my-tailnet.ts.net:8789 \
MAPPER_API_URL=http://usagi-api.my-tailnet.ts.net:8790 \
JOBS_API_URL=http://usagi-api.my-tailnet.ts.net:8790 \
bin/rails test test/integration/live_engine_smoke_test.rb test/integration/live_workflow_e2e_test.rb
```

## Production Secrets

Required Rails secrets:

```text
RAILS_MASTER_KEY
SECRET_KEY_BASE
DATABASE_URL
USAGI_API_SHARED_SECRET
```

Required API secrets:

```text
USAGI_API_AUTH_MODE=signed
USAGI_API_SHARED_SECRET
```

Use the same `USAGI_API_SHARED_SECRET` on Rails and every engine service.
Production engine services must not use `USAGI_API_AUTH_MODE=api_key` or
`disabled`.

## Artifact Provisioning

Production engine services require the `/data` layout documented in
`docs/api/artifacts.md`:

```text
/data/catalog/catalog.sqlite
/data/search/tantivy/index/
/data/search/sapbert/
/data/models/sapbert/
/data/models/thirawat-sapbert/
/data/mapper/thirawat-drug/
/data/jobs/jobs.sqlite
/data/jobs/results/
```

Do not commit these files. Provision them through a controlled artifact copy,
build job, or restore process before starting production engine services with
`USAGI_API_ENV=production`.

For build order and commands, see `docs/manual/api/05-artifact-builds.md`.
The THIRAWAT document embedding stage can run remotely on Modal; see
`infra/modal/README.md`. Artifact pack formats, Hugging Face Dataset guidance,
and OCI/Docker artifact image constraints are documented in
`docs/api/artifacts.md`.

## Backups

Back up these independently:

```text
PostgreSQL product database
Rails storage volume
engine /data volume
```

The PostgreSQL database contains users, projects, imports, mappings, review
state, audit events, comments, exports, and engine job mirrors. Rails storage
contains upload and export files plus Solid Queue/Cache/Cable SQLite files for
the single-node deployment. Engine `/data` contains generated artifacts and job
results owned by `usagi-api`.

Restore order:

```text
1. restore PostgreSQL
2. restore Rails storage
3. restore engine /data
4. start engine services
5. start Rails
6. run deployment smoke
```

## Rollback

Use Kamal rollback for Rails image rollback:

```bash
cd apps/web
bin/kamal rollback
```

Do not roll back the Rails database independently from product data without an
explicit migration/restore plan. Engine artifacts should be restored as a whole
`/data` snapshot when artifact compatibility is in doubt.

## Deployment Verification

Required before calling a deployment ready:

```bash
docker compose -f infra/docker/docker-compose.yml config
scripts/smoke-signed-auth.sh
scripts/smoke-deploy.sh
```

Rails gates:

```bash
cd apps/web
bin/rails test
bin/rubocop
bin/brakeman --no-pager
```

Rust gates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Host-side live workflow proof remains gated and should be run before claiming
end-to-end product readiness. With the full-stack compose file already running,
use the deployment smoke helper:

```bash
USAGI_DEPLOY_SMOKE_START=0 \
USAGI_DEPLOY_SMOKE_DEBUG_PORTS=1 \
USAGI_DEPLOY_SMOKE_RUN_LIVE_TESTS=1 \
scripts/smoke-deploy.sh
```

That re-applies the debug override if needed, keeps the stack running, and runs:

```text
live_engine_smoke_test.rb
live_workflow_e2e_test.rb
```

against the signed compose stack on `127.0.0.1:28788-28790`.

Manual equivalent:

```bash
cd apps/web
USAGI_LIVE_ENGINE=1 \
ENGINE_CLIENT_MODE=http \
USAGI_API_SHARED_SECRET=local-compose-shared-secret \
CATALOG_API_URL=http://127.0.0.1:28788 \
SEARCH_API_URL=http://127.0.0.1:28789 \
MAPPER_API_URL=http://127.0.0.1:28790 \
JOBS_API_URL=http://127.0.0.1:28790 \
bin/rails test test/integration/live_engine_smoke_test.rb test/integration/live_workflow_e2e_test.rb
```

### Verified Local E2E (2026-06-05)

On this repository state, the following passed against the local full-stack
compose file with fixture-backed `/data` artifacts:

```text
scripts/smoke-signed-auth.sh
USAGI_DEPLOY_SMOKE_START=1 scripts/smoke-deploy.sh
  -> {"catalog":"ready","search":"ready","mapper":"ready","signed_search_probe":"ok"}
USAGI_DEPLOY_SMOKE_START=0 USAGI_DEPLOY_SMOKE_DEBUG_PORTS=1 USAGI_DEPLOY_SMOKE_RUN_LIVE_TESTS=1 scripts/smoke-deploy.sh
  -> 6 runs, 67 assertions, 0 failures
```

Notes:

```text
First full-stack build from source can take several minutes.
Host-side live tests require docker-compose.debug.yml; base compose keeps engines on an internal network only.
If native usagi-api dev servers are already bound to 8788-8790, keep the debug defaults or stop the native servers.
Offline bin/rails test still requires bundle install under apps/web/.
```
