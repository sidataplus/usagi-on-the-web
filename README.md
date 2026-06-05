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

## Repo Layout

```text
apps/web/          Rails product workflow layer
services/          Rust HTTP services
crates/            Shared Rust crates
infra/docker/      Local Docker Compose deployment
infra/kamal/       Kamal/DigitalOcean operator notes
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
```

Do not commit runtime databases, indexes, model weights, or generated artifacts.
