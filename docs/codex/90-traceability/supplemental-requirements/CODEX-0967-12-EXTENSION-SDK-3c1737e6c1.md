# Supplemental Requirement: Plugin Boundary ADR Merge

Prompt ID: `CODEX-0967-12-EXTENSION-SDK-3c1737e6c1`
Primary Prompt: `CODEX-0946-12-EXTENSION-SDK-f6fbec755d`
Current-safe module: `extension_sdk::adr_0008_plugin_boundaries`

## Merge Result

P0: Plugin boundaries must state that extensions cannot directly append events, write databases, modify Authority Contract, forge dice, call LLM providers, or reveal restricted visibility.

P1: Boundary tests should keep proving authority mismatch, direct agent write, idempotency replay, expected-version conflict, and restricted visibility redaction.

## Boundary

This supplemental prompt owns no Rust src/test output. Implementation changes, if needed later, belong to the primary prompt above.

