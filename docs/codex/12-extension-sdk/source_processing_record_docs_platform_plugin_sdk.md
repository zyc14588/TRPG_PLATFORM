# Source Processing Record: Plugin SDK Platform Document

Prompt ID: `CODEX-0966-12-EXTENSION-SDK-7277a3f410`
Current-safe crate: `trpg-extension-sdk`
Current-safe module: `extension_sdk::source_processing_record_docs_platform_plugin_sdk`
Task type: `traceability-maintenance`
Output role: `documentation-or-traceability`

## Scope

This record preserves the plugin SDK platform document processing result for BATCH-045 without authorizing implementation output.

## Governance Boundaries

- Plugin SDK work must stay behind capability grants, Policy Gate, OpenFGA/OPA, audit, visibility, and fact provenance.
- Extension registration and compatibility checks are governed events, not direct database or Event Store writes by plugins.
- Historical source filenames and hashes stay provenance-only.

## Current-Safe Output

This Markdown file is the only current output for `CODEX-0966-12-EXTENSION-SDK-7277a3f410`.

## Test Responsibility

Coverage remains with `crates/trpg-extension-sdk/tests/s12_fixture_acceptance_contract_tests.rs` and the S12 command:

`cargo test -p trpg-extension-sdk --test s12_fixture_acceptance_contract_tests --all-features`

