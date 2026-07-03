# CI/CD Directory — v2.21 Canonical Workflow Sources

## Canonical extraction source

Only the following files are current workflow sources:

```text
ci-cd/workflows-extractable/target-ci.yml.md
ci-cd/workflows-extractable/target-contracts.yml.md
ci-cd/workflows-extractable/target-golden-scenarios.yml.md
ci-cd/workflows-extractable/target-docker-compose-smoke.yml.md
ci-cd/workflows-extractable/target-release.yml.md
```

Codex must extract the fenced YAML blocks from those files into `.github/workflows/*.yml`.

## Non-canonical historical files

Earlier `github-actions-*.yml.md` files are stored in:

```text
source-archive/provenance/**
```

They are provenance-only and must not be used as current workflow extraction inputs.
