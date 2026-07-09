# Ruleset Pack SDK Evidence

Prompt ID: `CODEX-0106-12-EXTENSION-SDK-34e4277c8c`
Module: `extension_sdk::ruleset_pack_sdk`

## Evidence

- Ruleset packs can register proposals but cannot forge official dice.
- Official dice authority remains server-only.
- Formal decisions continue through rules/workflow/event boundaries.

Tests: `cargo test -p trpg-extension-sdk --test ruleset_pack_sdk_contract_tests`
