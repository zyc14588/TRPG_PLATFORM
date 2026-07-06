# Source Processing Record: NATS Subject Contracts

Batch: BATCH-026
Prompt: P0067 / CODEX-0642-06-DATA-EVENTING-2b4817434a
Kind: documentation-or-traceability

## Current-Safe Output

- Output: docs/codex/06-data-eventing/source_processing_record_docs_contracts_nats_subjects.md
- Scope: traceability record only
- Implementation output: none

## Governance Assertions

- Current NATS subject names are declared through current-safe modules and tests.
- Required subjects include trpg.events.appended and trpg.projection.rebuild.requested.
- Subject contracts must carry visibility, fact provenance, correlation, causation, and authority contract version metadata.
- Historical source path tokens are provenance only and are not current module, event, metric, migration, or test names.

## Batch Evidence

- Work plan: evidence/batches/BATCH-026/WORK_PLAN.md
- Test record: evidence/batches/BATCH-026/TEST_RESULTS.md

