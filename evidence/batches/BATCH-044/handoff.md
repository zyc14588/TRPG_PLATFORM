# BATCH-044 Handoff

## Completed

- Added `trpg-extension-sdk` to the Cargo workspace.
- Implemented B044 primary modules:
  - `agent_pack_sdk`
  - `plugin_sdk`
  - `ruleset_pack_sdk`
  - `tool_provider_sdk`
  - `adr_0008_plugin_boundaries`
  - `extension_compatibility_matrix`
  - `sdk`
  - `readme`
- Added contract tests for all primary modules.
- Added S12 fixture tests for forbidden direct Event Store append, direct LLM access, direct LLM NATS denial, and compatibility report fields.
- Added docs-governance outputs for the S12 module overview and compatibility matrix.
- Added supplemental requirement records for B044 supplemental prompts.
- Added traceability source processing records for B044 traceability prompts.
- Added BATCH-044 evidence files.

## Next Batch Notes

- Do not re-run BATCH-044 unless metadata cleanup explicitly targets the primary-count mismatch.
- Future S12 work should build on `all_batch_044_contracts()` as the registry for current Extension SDK contracts.
- UI role snapshots, developer boundary screens, and any Node-based frontend checks remain outside this batch unless BATCH-045 explicitly owns them.
- Keep extension functionality behind Tool Grant, OpenFGA, OPA, Audit, Visibility, Fact Provenance, and Event Store governance.

## Open Items

- Upstream metadata says primary count `0`; normalized maps say primary count `8`.
- The workspace currently has no Node package manifest, so pnpm test/build checks are not executable.
