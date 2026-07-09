# Supplemental Requirement: CODEX-0851-10-TESTING-QUALITY-c4d5125cc0

Batch: `BATCH-038-10-testing-quality`  
Prompt file: `codex-prompts/10-testing-quality/P0023.md`  
Primary prompt: `CODEX-0091-10-TESTING-QUALITY-6730499fe0`  
Current module: `testing_quality::model_certification_tests`

## Boundary

This prompt is supplemental only and does not own Rust output.

## Merge Instructions

- Local model Level 4 is required for AI Keeper Orchestrator.
- Local-to-cloud fallback must be explicit, user-visible, snapshot-recorded, and auditable.

## Test Responsibility

Merged into `crates/trpg-testing/tests/model_certification_tests_contract_tests.rs`.
