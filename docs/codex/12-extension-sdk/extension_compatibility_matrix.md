# Extension Compatibility Matrix

Prompt ID: `CODEX-0104-12-EXTENSION-SDK-1523d32137`
Current-safe module: `extension_sdk::extension_compatibility_matrix`
Current-safe implementation owner: `CODEX-0947-12-EXTENSION-SDK-f162f72984`

## Contract

S12 extension compatibility is evaluated before an extension can be treated as loadable. The compatibility report must carry:

- `extension_id`
- `ruleset_version`
- `tool_schema_version`
- `compatibility_result`

The current B044 fixture accepts `ruleset_version = 7e` and `tool_schema_version = tool_schema.v1`.

## Governance Boundary

- Extensions cannot append Event Store events directly.
- Extensions cannot call OpenAI, Ollama, llama.cpp, or a local model endpoint directly.
- Extensions cannot write databases or mutate Authority Contract.
- Extensions cannot reveal `keeper_only`, `private_to_player`, `ai_internal`, `system_only`, or `system_private` data to unauthorized principals.
- Tool use must pass Tool Grant, OpenFGA, OPA, and Audit gates before a formal event is recorded.

## Evidence

Primary implementation lives in `crates/trpg-extension-sdk/src/extension_compatibility_matrix.rs`.
Contract tests live in `crates/trpg-extension-sdk/tests/extension_compatibility_matrix_contract_tests.rs` and `crates/trpg-extension-sdk/tests/extension_compatibility_matrix.rs`.
