# Supplemental Requirement: Plugin Capability Default Deny

Prompt ID: `CODEX-0952-12-EXTENSION-SDK-deb0e52024`
Primary Prompt: `CODEX-0105-12-EXTENSION-SDK-914ad16fbe`
Current-safe module: `extension_sdk::plugin_sdk`

## Merge Result

`ExtensionCapabilityGrantSet::default()` grants nothing. Plugin registration fails until requested capabilities are explicitly granted.

## Boundary

This supplemental prompt owns no Rust src/test output.
