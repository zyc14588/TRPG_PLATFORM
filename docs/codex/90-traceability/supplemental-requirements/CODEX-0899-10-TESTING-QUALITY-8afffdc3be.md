# Supplemental Requirement: Replay Consistency Tests

Prompt ID: `CODEX-0899-10-TESTING-QUALITY-8afffdc3be`
Primary Prompt: `CODEX-0092-10-TESTING-QUALITY-d6a006e0a1`
Shared module: `testing_quality::replay_consistency_tests`
Prompt file: `codex-prompts/10-testing-quality/P0069.md`

## Merge Instructions

- Replay tests must prove Event Store canon and projection rebuild safety.
- Visibility labels and fact provenance must survive replay.
- This supplemental prompt owns no Rust src/test output.
