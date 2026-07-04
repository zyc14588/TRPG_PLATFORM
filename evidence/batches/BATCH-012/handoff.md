# BATCH-012 Handoff

## Completed

- Added `trpg-runtime` as a workspace crate.
- Added B012 primary runtime modules using current-safe flat module filenames.
- Added a batch-level runtime contract test covering Tool Grant, Pending Decision, Decision Commit Pipeline, S06 fixtures, session, saga, scheduler, workflow, ADR boundary, visibility redaction, direct agent write rejection, expected version, and idempotency.
- Added target integration tests for `runtime_pending_decision` and `workflow_engine_contract`.
- Added docs-governance output `docs/codex/03-runtime-orchestration/m_03_runtime_orchestration.md`.
- Added supplemental traceability Markdown for the 10 supplemental prompts in this batch.
- Ran formatting, runtime tests, fixture JSON checks, cargo check, workspace tests, and clippy successfully.

## Not Done

- No SQLx migration, Axum handler, OpenAPI schema, NATS subject, WebSocket server, external workflow engine adapter, or provider integration was added.
- The current workspace still lacks later-stage crates such as data/eventing, agent runtime, API/realtime, and ruleset crates named in the top-level plan.

## Risks

- P1: `batch-prompts/start/B012.md` says primary count is 0, while `batches/B012.md` and the manifest identify 14 primary prompts. Operator should correct the batch prompt generator or annotate this as a known metadata defect.
- P2: Runtime currently uses an in-memory `EventStore` from `trpg-shared-kernel`; replace with S03 event-store adapter when `trpg-data-eventing` exists.
- P2: API/NATS/SQLx contract work remains for later S06/S08/S03 batches; this batch deliberately did not create those future integration surfaces.

## Next Batch

Proceed to `BATCH-013-03-runtime-orchestration` only after accepting the B012 evidence. Reuse `trpg-runtime::runtime_state_machines` instead of duplicating Command/Decision/Event/Visibility primitives.
