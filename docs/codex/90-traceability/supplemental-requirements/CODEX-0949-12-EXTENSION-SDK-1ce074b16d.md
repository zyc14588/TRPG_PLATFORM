# Supplemental Requirement: Plugin SDK Runtime Isolation

Prompt ID: `CODEX-0949-12-EXTENSION-SDK-1ce074b16d`
Primary Prompt: `CODEX-0105-12-EXTENSION-SDK-914ad16fbe`
Current-safe module: `extension_sdk::plugin_sdk`

## Merge Result

The primary `plugin_sdk` implementation rejects direct LLM, direct database write, direct Event Store append, and internal Tool Gate access through forbidden capabilities and contract tests.

## Boundary

This supplemental prompt owns no Rust src/test output.
