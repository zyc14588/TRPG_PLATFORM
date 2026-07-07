# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0717-07-API-REALTIME-CONTRACTS-fbe56925f9`
Primary Prompt: `CODEX-0067-07-API-REALTIME-CONTRACTS-1ccbeea1df`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::external_provider_contracts`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0048.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge external provider boundary requirements into the current-safe provider contract.
- Business services, API handlers, WebSocket handlers, rules code, and state services must not call OpenAI, Ollama, llama.cpp, or any bare model endpoint directly.
- Provider use must pass through Agent Gateway, Agent Orchestrator or Runtime, and Model Provider Adapter boundaries with explicit privacy and audit controls.
- Local models remain first-class providers, but unapproved local models cannot serve as AI Keeper Orchestrator and must not silently fall back across privacy boundaries.
- Provider output remains advisory until formalized through rules, state, and event-log gates.

## Test Responsibility

Coverage remains with `external_provider_contracts_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
