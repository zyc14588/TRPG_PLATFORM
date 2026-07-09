# Supplemental Requirement: CODEX-0844-10-TESTING-QUALITY-be04cff75f

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0015.md`  
Primary prompt: `CODEX-0093-10-TESTING-QUALITY-97f7f731a8`  
Current module: `testing_quality::test_strategy`

## Boundary

This prompt is supplemental only and does not own Rust output.

## Merge Instructions

- Keep test layers separated into module contract, stage fixture, and negative-case coverage.
- Require permission, version, idempotency, visibility, and prompt-injection negative cases.

## Test Responsibility

Merged into `crates/trpg-testing/tests/test_strategy_contract_tests.rs`.
