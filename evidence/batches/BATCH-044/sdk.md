# SDK Registry Evidence

Prompt ID: `CODEX-0953-12-EXTENSION-SDK-7588c965bd`
Module: `extension_sdk::sdk`

## Evidence

- The SDK registry exposes 8 B044 primary contracts.
- Contract names use normalized current-safe module/output naming.
- Registry events use the governed command envelope and Event Store append boundary.

Tests: `cargo test -p trpg-extension-sdk --test sdk_contract_tests`
