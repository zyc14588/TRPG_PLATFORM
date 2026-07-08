# 08 Platform Infrastructure

Current-safe implementation crate: `crates/trpg-platform`.

This module index records the B031 platform infrastructure contract layer. Runtime deployment wiring, concrete object storage services, admin health routes, and Docker Compose smoke execution are intentionally left to later S09 batches unless their current-safe prompts assign those outputs.

## B031 Primary Outputs

| Prompt ID | Module | Contract focus |
|---|---|---|
| CODEX-0074 | `background_workers` | Governed worker lifecycle event append |
| CODEX-0075 | `deployment_ops` | Deployment provider boundary and production secret checks |
| CODEX-0076 | `local_dev_environment` | Loopback-only local development service profile |
| CODEX-0077 | `object_storage` | Restricted object descriptor redaction |
| CODEX-0078 | `observability` | Current-safe metric names and restricted-detail redaction |
| CODEX-0079 | `performance_budget` | Fail-closed platform latency budget |
| CODEX-0724 | `deployment_observability` | Healthcheck-required deployment observation |
| CODEX-0728 | `reliability_performance` | Capped retry/backoff policy |
| CODEX-0732 | `observability_audit_trace` | Audit trace metadata and redaction guard |
| CODEX-0738 | `readme` | Platform infrastructure invariants |

## Governance Boundaries

- Platform modules do not call model providers directly.
- Formal writes use `CommandEnvelope -> EventStore`.
- Visibility labels and fact provenance stay attached to event envelopes.
- Restricted details are redacted from public-facing descriptors, metrics, deployment observations, and audit traces.
- Production deployment configuration rejects placeholder secrets and unauthenticated local provider exposure.

Evidence: `evidence/batches/BATCH-031/`.
