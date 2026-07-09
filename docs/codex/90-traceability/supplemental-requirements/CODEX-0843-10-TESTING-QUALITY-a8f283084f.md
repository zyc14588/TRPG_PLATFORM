# Supplemental Requirement: CODEX-0843-10-TESTING-QUALITY-a8f283084f

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0014.md`  
Primary prompt: `CODEX-0094-10-TESTING-QUALITY-6ac95ec41f`  
Current module: `testing_quality::testing_golden_ci`

## Boundary

This prompt is supplemental only and does not own Rust output.

## Merge Instructions

- Require S11 stage fixture coverage.
- Keep golden scenario, visibility leakage, export diff, and model certification gates visible in CI evidence.

## Test Responsibility

Merged into `crates/trpg-testing/tests/testing_golden_ci_contract_tests.rs`.
