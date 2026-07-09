# Supplemental Requirement: CODEX-0840-10-TESTING-QUALITY-069d3f779b

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0010.md`  
Primary prompt: `CODEX-0839-10-TESTING-QUALITY-09775e3a7b`  
Current module: `testing_quality::decision_trace_map`

## Boundary

This prompt is supplemental only. It does not own Rust source, tests, migrations, API handlers, event schemas, NATS subjects, metrics, or workflow names.

## Merge Instructions

- Keep all decision trace rows current-safe.
- Treat old source paths and hashes as provenance only.
- Verify all 25 B038 prompt IDs are represented in traceability evidence.

## Test Responsibility

Merged into `crates/trpg-testing/tests/decision_trace_map_contract_tests.rs`.
