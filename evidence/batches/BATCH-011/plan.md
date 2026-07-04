# BATCH-011 plan

Batch: `BATCH-011-02-domain-core -- Strict Governance Final`

Scope decision: all 6 prompts are `supplemental-requirement`; primary prompt count is 0. This batch may update supplemental Markdown and evidence only. It must not create or modify Rust implementation, tests, migrations, API handlers, NATS subjects, event schemas, metrics, workflows, or formal state write paths.

## Prompt work map

| Prompt ID | Current-safe module | Target file | Primary merge target | Allowed change | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-0329-02-DOMAIN-CORE-c958deea81` | `domain_core::command_cqrs_impl` | `codex-prompts/02-domain-core/P0103.md` | `CODEX-0281-02-DOMAIN-CORE-1e4096357b` | supplemental constraints, test assertions, traceability | command envelope, authority guard, idempotency replay, visibility/provenance replay assertions for primary-owned tests |
| `CODEX-0330-02-DOMAIN-CORE-b0fc555edb` | `domain_core::domain_model_impl` | `codex-prompts/02-domain-core/P0102.md` | `CODEX-0282-02-DOMAIN-CORE-b1fe69de22` | supplemental constraints, test assertions, traceability | Authority Contract immutability, fork-only change, confirmed fact provenance assertions for primary-owned tests |
| `CODEX-0331-02-DOMAIN-CORE-dbf31c8c58` | `domain_core::event_sourcing_projection_impl` | `codex-prompts/02-domain-core/P0104.md` | `CODEX-0283-02-DOMAIN-CORE-d29066e385` | supplemental constraints, test assertions, traceability | Event Store canon, replay determinism, expected_version conflict, redaction assertions for primary-owned tests |
| `CODEX-0332-02-DOMAIN-CORE-6ccef95407` | `domain_core::investigation_clue_npc_time_impl` | `codex-prompts/02-domain-core/P0100.md` | `CODEX-0284-02-DOMAIN-CORE-370ec69864` | supplemental constraints, test assertions, traceability | core clue fail-forward, split-party visibility, NPC claim provenance assertions for primary-owned tests |
| `CODEX-0333-02-DOMAIN-CORE-7fdd89160b` | `domain_core::rule_runtime_coc7_impl` | `codex-prompts/02-domain-core/P0101.md` | `CODEX-0285-02-DOMAIN-CORE-d1c3bee3b7` | supplemental constraints, test assertions, traceability | server-side dice, SAN/combat/chase audit chain, ad-hoc ruling assertions for primary-owned tests |
| `CODEX-0334-02-DOMAIN-CORE-f95a64393d` | `domain_core::visibility_fact_provenance_impl` | `codex-prompts/02-domain-core/P0106.md` | `CODEX-0286-02-DOMAIN-CORE-590846948a` | supplemental constraints, test assertions, traceability | visibility lattice, leakage negatives, confirmed fact provenance assertions for primary-owned tests |

## Checks to run

1. Minimum related checks:
   - verify BATCH-011 prompt IDs map to current-safe outputs.
   - verify each supplemental file has a BATCH-011 merge packet and primary merge target.
   - verify changed files are limited to the six supplemental prompts and `evidence/batches/BATCH-011/`.
2. Stage-related checks:
   - `cargo test -p trpg-domain-core --all-features`
   - `cargo test -p trpg-domain-core authority --all-features`
   - `cargo test -p trpg-domain-core visibility --all-features`

## Non-goals

- No Rust implementation or test edits in this batch.
- No migration, schema, NATS, metric, workflow, API, or event-name changes.
- No use of `source-archive/**` as a current implementation source.
