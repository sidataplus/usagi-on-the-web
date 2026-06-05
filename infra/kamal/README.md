# Kamal Deployment Notes

These notes support `apps/web/config/deploy.yml`. The source of truth for the
deployment procedure is `docs/manual/web/04-deployment.md`.

For **AWS EC2 + RDS PostgreSQL**, see `infra/aws/README.md` and the **AWS With
RDS PostgreSQL** section in the deployment manual.

## DigitalOcean Checklist

Prepare:

```text
DigitalOcean droplet or droplets
GitHub Container Registry package access
DNS record for APP_HOST
firewall allowing public 80/443 only to Rails
persistent volume or backup target for Rails storage
persistent volume or backup target for engine /data
```

Do not expose engine ports publicly. `catalog-api`, `search-api`, `mapper-api`,
and `api-worker` should be reachable only on the Kamal/Docker private network or
through host-local bindings used by accessories.

## Droplet Sizing Summary

Full planning detail lives in `docs/manual/web/04-deployment.md` under
**DigitalOcean Droplet Sizing**. Quick picks:

| Topology | DigitalOcean shape |
|---|---|
| Rails only; API over Tailscale | **4 GB / 2 vCPU** web droplet + Managed Postgres |
| Rails + all API accessories on one host | **32 GB / 8 vCPU** + **160 GB** `/data` volume |
| Split on DigitalOcean | **4 GB / 2 vCPU** web + Managed Postgres; **32 GB / 8 vCPU** engine + **160 GB** volume |
| API compute off DO | size the remote host like the engine row above |

These estimates assume a full Athena standard-concept artifact set and a small
internal mapping team, not fixture-mini dev data or public SaaS load.

## Local API Compute Over Tailscale

If the API services run on a separate local compute server, connect the
DigitalOcean Rails host and the local server with Tailscale. Set the engine URLs
before running Kamal:

```bash
export CATALOG_API_URL=http://usagi-api.my-tailnet.ts.net:8788
export SEARCH_API_URL=http://usagi-api.my-tailnet.ts.net:8789
export MAPPER_API_URL=http://usagi-api.my-tailnet.ts.net:8790
export JOBS_API_URL=http://usagi-api.my-tailnet.ts.net:8790
```

For this topology, remove or disable the engine accessories in
`apps/web/config/deploy.yml` for the deploy environment. Rails still needs
`USAGI_API_SHARED_SECRET`, and the local API services must use the same secret
with `USAGI_API_AUTH_MODE=signed`.

On the local API server, bind the API-only Compose stack to the Tailscale
interface and deny public ingress:

```bash
export USAGI_PUBLISH_HOST=100.x.y.z
export USAGI_API_ENV=production
export USAGI_API_AUTH_MODE=signed
export USAGI_API_SHARED_SECRET=...

docker compose -f infra/docker/docker-compose.api.yml up -d --build
```

Build and maintain `/data` artifacts on the local server with
`docs/manual/api/05-artifact-builds.md`.

## Required Images

Push the Rails image through Kamal. Build and push engine images separately:

```text
ghcr.io/sidataplus/usagi-on-the-web/rails-web:latest
ghcr.io/sidataplus/usagi-on-the-web/catalog-api:latest
ghcr.io/sidataplus/usagi-on-the-web/search-api:latest
ghcr.io/sidataplus/usagi-on-the-web/mapper-api:latest
ghcr.io/sidataplus/usagi-on-the-web/api-worker:latest
```

The repo's `Container Images` GitHub Actions workflow publishes those images to
GHCR on `main` and by manual dispatch. For Kamal, set:

```bash
export GHCR_USERNAME=sidataplus
export GHCR_IMAGE_PREFIX=sidataplus/usagi-on-the-web
export USAGI_IMAGE_TAG=latest
```

## Required Secrets

Kamal reads secrets from `apps/web/.kamal/secrets`, which expects these
environment variables:

```text
GHCR_TOKEN
RAILS_MASTER_KEY
SECRET_KEY_BASE
DATABASE_URL
POSTGRES_PASSWORD
USAGI_API_SHARED_SECRET
```

Use a password manager or CI secret store. Do not commit raw secret values or
`config/master.key`.

`GHCR_TOKEN` must be able to read packages for deploys. It also needs package
write permissions when a machine is expected to push images.

## Artifact Volume

Engine accessories mount `/data`. Provision the contents documented in
`docs/api/artifacts.md` before setting `USAGI_API_ENV=production` on engine
services:

```text
catalog SQLite
Tantivy index
SapBERT index/model files
THIRAWAT model files
Tachiom/mapper artifacts
jobs SQLite and result directory
```

Rails must never read these files directly.

## Smoke

After deploy:

```bash
cd apps/web
bin/kamal app exec "bin/rails runner 'puts DeploymentChecks.production_errors.inspect'"
bin/kamal app logs
```

From the repo root on a controlled operator machine or CI runner, run:

```bash
scripts/smoke-signed-auth.sh
scripts/smoke-deploy.sh
```

For production, adapt `scripts/smoke-deploy.sh` to the public `APP_HOST` and keep
engine probes on the private side of the network.
