# Plugin SDK Evidence

Prompt ID: `CODEX-0105-12-EXTENSION-SDK-914ad16fbe`
Module: `extension_sdk::plugin_sdk`

## Evidence

- Plugin capabilities are default deny.
- Direct LLM, database write, Event Store append, Tool Gate bypass, Authority Contract mutation, dice forging, and visibility leak capabilities are forbidden.
- Tool Grant, OpenFGA, OPA, and Audit gates must pass before recording plugin events.

Tests: `cargo test -p trpg-extension-sdk --test plugin_sdk_contract_tests`
