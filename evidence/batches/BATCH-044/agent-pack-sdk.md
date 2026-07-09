# Agent Pack SDK Evidence

Prompt ID: `CODEX-0103-12-EXTENSION-SDK-1322493559`
Module: `extension_sdk::agent_pack_sdk`

## Evidence

- Agent pack commands record through the shared Extension SDK Event Store append boundary.
- Keeper orchestration requires the Agent Gateway / Orchestrator / Runtime boundary and provider certification level 4 or higher.
- Direct model access is not exposed by the agent pack manifest.

Tests: `cargo test -p trpg-extension-sdk --test agent_pack_sdk_contract_tests`
