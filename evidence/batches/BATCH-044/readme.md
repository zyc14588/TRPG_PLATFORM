# Extension SDK Readme Evidence

Prompt ID: `CODEX-0957-12-EXTENSION-SDK-2c33b70efe`
Module: `extension_sdk::readme`

## Evidence

- Shared capability, policy gate, compatibility report, redaction, and contract registry helpers are centralized in `readme`.
- Restricted visibility labels redact in exported/replayed outputs.

Tests: `cargo test -p trpg-extension-sdk --test readme_contract_tests`
