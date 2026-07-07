# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0701-07-API-REALTIME-CONTRACTS-12efb1a65d`
Primary Prompt: `CODEX-0070-07-API-REALTIME-CONTRACTS-40bb6959f3`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::realtime_sync`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0027.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge realtime replay, reconnect, split-party, and multi-room synchronization requirements into the existing current-safe `realtime_sync` module contract.
- Preserve Event Store authority: projections, cache, summaries, WebSocket views, and realtime fanout remain rebuildable read models.
- Require principal-specific visibility filtering before any delta, replay, room sync, or reconnect payload is emitted.
- Preserve fact provenance, causation, correlation, request idempotency, and actor metadata across realtime outputs.
- Keep provider and model access outside this module; no direct OpenAI, Ollama, llama.cpp, or bare model calls are permitted here.

## Test Responsibility

Coverage remains with `realtime_sync_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
