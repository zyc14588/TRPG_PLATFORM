# Source Processing Record: ADR 0008 Plugin Boundaries

Prompt ID: `CODEX-0960-12-EXTENSION-SDK-df830c1c96`
Prompt file: `codex-prompts/12-extension-sdk/P0021.md`
Current-safe output: `docs/codex/12-extension-sdk/source_processing_record_docs_adr_adr_0008_plugin_boundaries.md`

## Boundary

- Documentation-or-traceability output only.
- No Rust module, migration, event schema, metric, workflow, or test name is created by this prompt.
- Implementation ownership remains with `CODEX-0946-12-EXTENSION-SDK-f6fbec755d`.

## Covered Current Module

`extension_sdk::adr_0008_plugin_boundaries` forbids direct LLM access, direct database writes, Event Store append bypass, internal Tool Gate access, Authority Contract mutation, forged dice, and restricted visibility disclosure.
