# Supplemental Requirement Merge Instruction

Prompt ID: `CODEX-0720-07-API-REALTIME-CONTRACTS-f03793d656`
Primary Prompt: `CODEX-0070-07-API-REALTIME-CONTRACTS-40bb6959f3`
Batch: `BATCH-030-07-api-realtime-contracts`
Target module: `api_realtime_contracts::realtime_sync`
Source prompt: `codex-prompts/07-api-realtime-contracts/P0047.md`

## Boundary

This supplemental prompt is not an independent Rust source, test, migration, API handler, NATS subject, metric, workflow, or formal state write owner.

## Merge Instruction

- Merge additional realtime sync governance into the current-safe `realtime_sync` contract.
- Realtime delivery must be deterministic from canonical event sequence, idempotent on reconnect, and filtered per principal before transport emission.
- Split-party, multi-scene, and spectator streams must not share private or Keeper-only material across visibility labels.
- Projection lag, replay cursor, and event ordering metadata must be auditable.
- Realtime sync remains a transport/read-model contract, not a formal state writer.

## Test Responsibility

Coverage remains with `realtime_sync_contract_tests`, `s08_fixture_acceptance_contract_tests`, and the S08 stage command for `trpg-api`.
