# Supplemental Requirement: CODEX-0853-10-TESTING-QUALITY-eaf9de3475

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0024.md`  
Primary prompt: `CODEX-0092-10-TESTING-QUALITY-d6a006e0a1`  
Current module: `testing_quality::replay_consistency_tests`

## Boundary

This prompt is supplemental only and does not own Rust output.

## Merge Instructions

- Event Store remains canonical.
- Projection, summary, export, and diff reports must be rebuildable from events.

## Test Responsibility

Merged into `crates/trpg-testing/tests/replay_consistency_tests_contract_tests.rs`.
