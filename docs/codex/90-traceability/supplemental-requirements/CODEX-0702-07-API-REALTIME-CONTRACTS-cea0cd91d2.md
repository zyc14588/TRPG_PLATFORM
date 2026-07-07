# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0702-07-API-REALTIME-CONTRACTS-cea0cd91d2`
Primary Prompt: `CODEX-0071-07-API-REALTIME-CONTRACTS-3277264d0e`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::request_idempotency_contract`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0028.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge idempotent command submission requirements into the existing current-safe request idempotency contract.
- Require idempotency keys, actor identity, authority mode, expected version, correlation id, causation id, visibility label, and fact provenance on formal command envelopes.
- Replays must return the same accepted decision or conflict result without appending duplicate canonical events.
- Conflict handling must preserve the formal Command -> Workflow -> Decision -> Event Store -> Projection path.
- No agent, API handler, WebSocket handler, or provider adapter may bypass idempotency to write formal state.

## Test Responsibility

Coverage remains with `request_idempotency_contract_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
