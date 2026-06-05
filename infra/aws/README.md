# AWS Deployment Notes

These notes support Kamal deployments on **EC2** with **RDS PostgreSQL** (or
Aurora PostgreSQL). The source of truth for product deployment shape is
`docs/manual/web/04-deployment.md`.

DigitalOcean droplet sizing estimates in that manual translate to EC2/RDS sizes
in the **AWS With RDS PostgreSQL** section.

## Quick Topology Picks

| Goal | AWS shape |
|---|---|
| Cheapest production AWS path | **t3.medium** web EC2 + **db.t4g.medium** RDS + API over Tailscale |
| Engines stay in AWS | **t3.medium** web + **db.t4g.medium** RDS + **r6i.2xlarge** engine EC2 |
| All services on one box | **r6i.2xlarge** EC2 + RDS (disable co-located Postgres accessory) |

## Prerequisites

```text
VPC with public + private subnets
EC2 instance(s) for Kamal
RDS PostgreSQL in private subnets
EBS volume for Rails storage (usagi_rails_storage)
EBS volume for engine /data when engines run on EC2
Route 53 or external DNS for APP_HOST
GHCR pull access (or ECR mirror)
Secrets in SSM Parameter Store, Secrets Manager, or CI secret store
```

## Disable The Kamal Postgres Accessory

`apps/web/config/deploy.yml` includes an `accessories.db` block for a co-located
Postgres container. For RDS:

```text
1. Remove or comment out accessories.db before deploy
2. Provide DATABASE_URL as a Kamal secret
3. Do not rely on the Docker hostname db:
```

Rails, Solid Queue, Solid Cache, and Solid Cable still use SQLite files under
`/rails/storage` on the web instance EBS volume. Back those up separately from
RDS.

## DATABASE_URL

Use SSL for RDS:

```bash
export DATABASE_URL="postgres://usagi:${DB_PASSWORD}@usagi-prod.abc123.us-east-1.rds.amazonaws.com:5432/usagi_production?sslmode=require"
```

Initialize once:

```bash
cd apps/web
bin/kamal app exec "bin/rails db:prepare"
```

Or restore from `pg_dump` before cutover.

## RDS Sizing

Planning estimates for a small internal mapping team with full Athena artifacts
on a separate engine host:

| Tier | Instance | Storage | Notes |
|---|---|---|---|
| Pilot | db.t4g.small | 20 GB gp3 | tight; dev/staging only |
| Recommended | db.t4g.medium | 20-50 GB gp3 | matches DO Managed Postgres 2-4 GB class |
| Growth | db.t4g.large | 50 GB gp3 | heavier imports, audit, exports |

Prefer **RDS PostgreSQL** over Aurora unless you need Multi-AZ failover or read
replicas now. Enable automated backups and PITR.

## EC2 Sizing

| Role | Instance | EBS |
|---|---|---|
| Rails web | t3.medium or t3.large | 25-50 GB gp3 |
| Engine host | r6i.2xlarge or m6i.2xlarge | 160 GB gp3 for /data |

Kamal `builder.arch` is `amd64` in `deploy.yml`. Use x86 instance families unless
you publish arm64 images.

## Security Groups

Create three security groups when split across web + engine + RDS:

### `usagi-web-sg`

Inbound:

```text
443, 80 from 0.0.0.0/0 (or from ALB security group only)
22 from operator IP (optional)
```

Outbound:

```text
5432 to usagi-rds-sg
8788-8790 to usagi-engine-sg
443 for GHCR/ECR and package updates
```

### `usagi-rds-sg`

Inbound:

```text
5432 from usagi-web-sg only
```

No public inbound.

### `usagi-engine-sg`

Inbound:

```text
8788, 8789, 8790 from usagi-web-sg only
22 from operator IP (optional)
```

No public inbound on engine ports.

## Engine URLs On AWS

When `catalog-api`, `search-api`, and `mapper-api` run as Kamal accessories on a
second EC2 host, Rails cannot use Docker service names like `catalog-api:8788`
across hosts. Export private VPC URLs instead:

```bash
export KAMAL_ENGINE_HOST=10.0.2.20
export CATALOG_API_URL=http://10.0.2.20:8788
export SEARCH_API_URL=http://10.0.2.20:8789
export MAPPER_API_URL=http://10.0.2.20:8790
export JOBS_API_URL=http://10.0.2.20:8790
```

`DeploymentChecks` accepts RFC1918 private IPs. Tailscale URLs remain valid if
engines stay off AWS.

## Kamal Deploy Env

```bash
export KAMAL_WEB_HOST=203.0.113.10
export KAMAL_ENGINE_HOST=10.0.2.20
export APP_HOST=usagi.example.org
export APP_HOSTS=usagi.example.org
export GHCR_USERNAME=sidataplus
export GHCR_IMAGE_PREFIX=sidataplus/usagi-on-the-web
export USAGI_IMAGE_TAG=latest

export GHCR_TOKEN=...
export RAILS_MASTER_KEY=...
export SECRET_KEY_BASE=...
export DATABASE_URL=...
export USAGI_API_SHARED_SECRET=...
```

Deploy:

```bash
cd apps/web
bin/kamal config
bin/kamal setup
bin/kamal deploy
```

## TLS Options

**Default (simplest):** Kamal proxy on the web EC2 instance with Let's Encrypt,
`FORCE_SSL=true`, public DNS to the instance.

**More AWS-native:** Application Load Balancer + ACM certificate, target group to
the web instance, `FORCE_SSL=true` if the ALB terminates HTTPS and forwards HTTP
or HTTPS to the instance. Match `production.rb` `assume_ssl` / `force_ssl` to the
actual hop configuration.

## Registry

Current images publish to GHCR via `.github/workflows/container-images.yml`.
That works from EC2 with `GHCR_TOKEN`.

Optional: mirror images to **ECR** in the same region to reduce pull latency and
egress.

## Backups

| Asset | Method |
|---|---|
| RDS | automated snapshots, PITR |
| Rails EBS (`/rails/storage`) | AWS Backup or scheduled EBS snapshots |
| Engine EBS (`/data`) | EBS snapshots |

Restore order:

```text
1. restore RDS
2. restore Rails EBS
3. restore engine /data
4. start engine accessories
5. start Rails
6. run deployment smoke
```

## Tailscale On EC2

Production Option 3 works on AWS the same way as DigitalOcean:

```text
Rails on EC2 + RDS in VPC
API compute on-prem or another host over Tailscale
```

Install Tailscale on the web EC2 instance. Disable engine accessories in
`deploy.yml`. Set engine URLs to `*.ts.net` or `100.x.y.z` addresses.

## Smoke After Deploy

```bash
cd apps/web
bin/kamal app exec "bin/rails runner 'puts DeploymentChecks.production_errors.inspect'"
bin/kamal app logs
```

From an operator machine:

```bash
scripts/smoke-signed-auth.sh
```

Adapt full-stack `scripts/smoke-deploy.sh` to `APP_HOST` for production HTML
smoke. Keep engine probes on the private network side.

## Cost Guide (Order Of Magnitude)

Prices vary by region and change over time. For planning:

```text
t3.medium EC2             ~ $30/mo
db.t4g.medium RDS         ~ $55/mo
r6i.2xlarge engine EC2    ~ $350/mo
160 GB gp3 EBS            ~ $15/mo per volume
```

Often more than a minimal DigitalOcean stack, but RDS backups, VPC isolation,
and IAM integrate cleanly with institutional AWS accounts.
