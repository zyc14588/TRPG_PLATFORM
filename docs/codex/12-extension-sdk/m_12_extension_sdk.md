# M12 Extension SDK

Prompt ID: `CODEX-0102-12-EXTENSION-SDK-77b0c49963`
Current-safe module: `extension_sdk::m_12_extension_sdk`

## Batch Scope

BATCH-044 establishes the first current-safe Extension SDK boundary crate: `trpg-extension-sdk`.

The batch implements the current primary owners for:

- `agent_pack_sdk`
- `plugin_sdk`
- `ruleset_pack_sdk`
- `tool_provider_sdk`
- `adr_0008_plugin_boundaries`
- `extension_compatibility_matrix`
- `sdk`
- `readme`

## Non-Negotiable Boundaries

- All formal writes remain `Command -> Workflow -> Decision -> Event Store -> Projection`.
- Extension code never owns the Event Store append path.
- Plugin and tool-provider capability grants are default deny.
- Direct LLM access, direct database writes, Authority Contract mutation, forged dice, and visibility leaks are forbidden capabilities.
- Compatibility records preserve `extension_id`, `ruleset_version`, `tool_schema_version`, and `compatibility_result`.

## Tests

The S12 SDK boundary is covered by `cargo test -p trpg-extension-sdk --all-features`, plus the stage compatibility matrix test target.
