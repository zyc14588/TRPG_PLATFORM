# ADR 0008 Plugin Boundaries Evidence

Prompt ID: `CODEX-0946-12-EXTENSION-SDK-f6fbec755d`
Module: `extension_sdk::adr_0008_plugin_boundaries`

## Evidence

- Current boundary policy rejects direct LLM, direct DB, Event Store append bypass, internal Tool Gate access, Authority Contract mutation, dice forging, and restricted visibility disclosure.

Tests: `cargo test -p trpg-extension-sdk --test adr_0008_plugin_boundaries_contract_tests`
