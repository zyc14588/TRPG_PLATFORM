# BATCH-017 Handoff

## Completed

- Added the `trpg-agent-runtime` workspace crate.
- Implemented strict AI agent governance contracts for tool permission, HUMAN_KP draft-only behavior, AI_KP orchestrator adjudication permission, direct formal-write denial, prompt-injection redaction, provider boundary validation, local model certification, and RAG visibility filtering.
- Reused `trpg-shared-kernel` authority, visibility, provenance, command envelope, and event store types.
- Added BATCH-017 contract tests and ran minimal, stage/workspace, formatting, patch, and clippy checks.
- Wrote batch evidence under `evidence/batches/BATCH-017/`.

## Not Started

- No later batch was started.
- No API handlers, migrations, NATS subjects, provider network clients, database writes, or frontend code were added in this batch.

## Remaining Risks

- The B017 start prompt reports primary prompt count 0, while the current-safe maps identify 16 primary outputs. This is recorded as a batch metadata inconsistency, not an implementation blocker.
- S07 test-plan examples mention specific future test names such as `agent_tool_permission_gate` and `model_certification_tests`; this batch used one focused contract test file instead, covering the same governance responsibilities.
- The new runtime crate is an in-memory governance contract layer. Future batches that add API/workflow/provider adapters must keep these boundaries and tests intact.
- Cargo prints a non-failing canonicalization warning for `C:\Users\zyc14588` in this environment.

## Next Batch Notes

- Build future agent-facing API/workflow/provider adapters on top of `trpg-agent-runtime` rather than duplicating governance logic.
- Keep all AI invocation behind `Agent Gateway -> Agent Orchestrator/Runtime -> Model Provider Adapter`.
- Preserve the rule that supplemental prompts merge into existing primary modules instead of creating new Rust outputs.
- Any future persistence or integration work must route formal decisions through command, workflow, decision, event store, and projection paths.
