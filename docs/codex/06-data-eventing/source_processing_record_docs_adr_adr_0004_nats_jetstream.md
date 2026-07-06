# Source Processing Record: ADR 0004 NATS JetStream

Batch: BATCH-026
Prompt: P0065 / CODEX-0640-06-DATA-EVENTING-bc0a025013
Kind: documentation-or-traceability

## Current-Safe Output

- Output: docs/codex/06-data-eventing/source_processing_record_docs_adr_adr_0004_nats_jetstream.md
- Scope: traceability record only
- Implementation output: none

## Governance Assertions

- NATS subjects are derived integration surfaces, not canonical state.
- Publish operations are driven by event_outbox records produced after governed Event Store append.
- Retry and dead-letter handling must preserve idempotency, correlation, causation, visibility, and fact provenance.
- Cross-boundary delivery cannot weaken Authority Contract or visibility policy.

## Batch Evidence

- Work plan: evidence/batches/BATCH-026/WORK_PLAN.md
- Test record: evidence/batches/BATCH-026/TEST_RESULTS.md

