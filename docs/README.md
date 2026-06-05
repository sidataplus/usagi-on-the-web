# Usagi-on-the-Web v3 Rails Development Docs

This bundle contains the Rails v3 implementation spec and the companion docs needed to begin development without turning the project into a fog machine with a Gemfile.

## Included files

```text
docs/rails/implementation-spec.md
docs/rails/00_development-plan.md
docs/rails/01_product-shape.md
docs/rails/02_domain-model.md
docs/rails/03_routes-controllers-views.md

docs/security/rails-api-boundary.md

docs/testing/contract-fixtures.md

docs/manual/api/00-deployment.md
docs/manual/api/05-artifact-builds.md
docs/manual/web/03-verification.md
docs/manual/web/04-deployment.md
infra/aws/README.md
infra/modal/README.md
```

## Reading order

1. `../AGENTS.md`
2. `docs/rails/implementation-spec.md`
3. `docs/rails/00_development-plan.md`
4. `docs/rails/01_product-shape.md`
5. `docs/rails/02_domain-model.md`
6. `docs/rails/03_routes-controllers-views.md`
7. `docs/security/rails-api-boundary.md`
8. `docs/testing/contract-fixtures.md`
9. `docs/manual/web/04-deployment.md`
10. `docs/manual/api/05-artifact-builds.md`

## Core rule

Rails owns product workflow state. `usagi-api` owns engine internals. The browser talks to Rails. Rails talks to `usagi-api` over a private, signed boundary.
