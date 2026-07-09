# Tool Provider SDK Evidence

Prompt ID: `CODEX-0107-12-EXTENSION-SDK-18948e0a9e`
Module: `extension_sdk::tool_provider_sdk`

## Evidence

- Tool provider manifests must return Visibility Label and Fact Provenance.
- Invocation requires Tool Grant, OpenFGA, OPA, and Audit.
- Direct model/provider fallback is not exposed by the SDK.

Tests: `cargo test -p trpg-extension-sdk --test tool_provider_sdk_contract_tests`
