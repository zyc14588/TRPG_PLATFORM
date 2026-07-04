# BATCH-009 Handoff

BATCH-009 is complete for the current S02 domain-core scope.

## What Landed

- 8 current-safe BATCH-009 primary modules were added under `crates/trpg-domain-core/src/`.
- 8 matching contract test files were added under `crates/trpg-domain-core/tests/`.
- 17 traceability-only source-processing records were added under `docs/codex/02-domain-core/`.
- Batch evidence was recorded under `evidence/batches/BATCH-009/`.

## Handoff Notes

- Later batches should reuse these current-safe facades instead of adding aliases based on historical source paths.
- Real database-backed Event Store, API handlers, NATS subjects, workflow engines, OpenFGA/OPA adapters, and AI/provider boundaries remain later-stage responsibilities.
- Keep the existing invariant: Agent output can propose or draft, but formal canon still requires command envelope, workflow/rules/tool decision, event append, projection replay, visibility enforcement, and fact provenance.

## Known Residual Risk

- The implementation remains in-memory domain-core scaffolding because BATCH-009 does not own persistence, runtime orchestration, API, or provider integration.
- Cargo emitted a path canonicalization warning for `C:\Users\zyc14588` during filtered checks, but all commands exited successfully.
