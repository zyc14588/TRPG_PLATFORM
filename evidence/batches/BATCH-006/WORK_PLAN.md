# BATCH-006 Work Plan

- Batch: BATCH-006-01-foundation - Strict Governance Final
- Declared prompt count: 23
- Repair scope: implement only the two active primary prompts from B006.
- Active primary prompts: CODEX-0216 and CODEX-0222.
- Scope guard: source-archive/** is provenance only; no historical source path, version marker, hash, or previous-version token may become a current module, migration, event, metric, workflow, test, or output name.

## Prompt Mapping

| Prompt ID | Prompt | Batch role | Effective B006 action | Current-safe module | Target files | Allowed scope | Test responsibility |
| --- | --- | --- | --- | --- | --- | --- | --- |
| CODEX-0214-01-FOUNDATION-79c289242e | P0076.md | documentation-or-traceability | executed | shared_kernel::readme_processed | docs/codex/01-foundation/readme_processed.md | Markdown traceability record only. | Prompt coverage plus S01 checks. |
| CODEX-0215-01-FOUNDATION-411484506a | P0077.md | documentation-or-traceability | executed | shared_kernel::source_processing_record_sources_latest_deep_research_rust_summary | docs/codex/01-foundation/source_processing_record_sources_latest_deep_research_rust_summary.md | Markdown traceability record only. | Prompt coverage plus S01 checks. |
| CODEX-0216-01-FOUNDATION-d39bc6ef34 | P0078.md | primary-implementation | executed | shared_kernel::adr_0001_rust_first | crates/trpg-shared-kernel/src/adr_0001_rust_first.rs; crates/trpg-shared-kernel/tests/adr_0001_rust_first_contract_tests.rs | Primary Rust src/test only for this current-safe module. | Target contract test plus S01 cargo checks. |
| CODEX-0217-01-FOUNDATION-6163fad1ce | P0079.md | supplemental-requirement | recorded only | shared_kernel::cargo_workspace | codex-prompts/01-foundation/P0079.md | Supplemental constraint only; no Rust src/test, migration, API handler, NATS subject, workflow, event schema, or metric label. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0218-01-FOUNDATION-3659d645a7 | P0080.md | supplemental-requirement | recorded only | shared_kernel::constitution | codex-prompts/01-foundation/P0080.md | Supplemental constraint only; merge target CODEX-0164 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0219-01-FOUNDATION-e312e3694b | P0081.md | supplemental-requirement | recorded only | shared_kernel::document_set | codex-prompts/01-foundation/P0081.md | Supplemental constraint only; merge target CODEX-0165 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0220-01-FOUNDATION-e6fa26b126 | P0082.md | supplemental-requirement | recorded only | shared_kernel::open_source_reference_matrix | codex-prompts/01-foundation/P0082.md | Supplemental constraint only; merge target CODEX-0161 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0221-01-FOUNDATION-e2b2757285 | P0083.md | supplemental-requirement | recorded only | shared_kernel::system_context | codex-prompts/01-foundation/P0083.md | Supplemental constraint only; merge target CODEX-0166 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0222-01-FOUNDATION-c1599cb21d | P0084.md | primary-implementation | executed | shared_kernel::technology_selection_rust | crates/trpg-shared-kernel/src/technology_selection_rust.rs; crates/trpg-shared-kernel/tests/technology_selection_rust_contract_tests.rs | Primary Rust src/test only for this current-safe module. | Target contract test plus S01 cargo checks. |
| CODEX-0223-01-FOUNDATION-809f614768 | P0085.md | supplemental-requirement | recorded only | shared_kernel::cargo_workspace | codex-prompts/01-foundation/P0085.md | Supplemental constraint only; merge target CODEX-0016 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0224-01-FOUNDATION-b690e9cf59 | P0086.md | supplemental-requirement | recorded only | shared_kernel::config_model | codex-prompts/01-foundation/P0086.md | Supplemental constraint only; merge target CODEX-0017 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0225-01-FOUNDATION-1d889d2a5d | P0087.md | supplemental-requirement | recorded only | shared_kernel::crate_ownership | codex-prompts/01-foundation/P0087.md | Supplemental constraint only; merge target CODEX-0018 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0226-01-FOUNDATION-6ba5e9acb1 | P0088.md | supplemental-requirement | recorded only | shared_kernel::dependency_direction | codex-prompts/01-foundation/P0088.md | Supplemental constraint only; merge target CODEX-0019 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0227-01-FOUNDATION-52cad60d03 | P0089.md | supplemental-requirement | recorded only | shared_kernel::error_model | codex-prompts/01-foundation/P0089.md | Supplemental constraint only; merge target CODEX-0020 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0228-01-FOUNDATION-f021763065 | P0090.md | supplemental-requirement | recorded only | shared_kernel::readme | codex-prompts/01-foundation/P0090.md | Supplemental constraint only; merge target CODEX-0178 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0229-01-FOUNDATION-979f0dbec3 | P0091.md | supplemental-requirement | recorded only | shared_kernel::rust_coding_model | codex-prompts/01-foundation/P0091.md | Supplemental constraint only; merge target CODEX-0021 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0230-01-FOUNDATION-dd68b1a909 | P0092.md | supplemental-requirement | recorded only | shared_kernel::shared_kernel | codex-prompts/01-foundation/P0092.md | Supplemental constraint only; merge target CODEX-0022 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0231-01-FOUNDATION-656bf64610 | P0093.md | supplemental-requirement | recorded only | shared_kernel::cargo_workspace_impl | codex-prompts/01-foundation/P0093.md | Supplemental constraint only; merge target CODEX-0186 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0232-01-FOUNDATION-66823f36cd | P0094.md | supplemental-requirement | recorded only | shared_kernel::constitution_impl | codex-prompts/01-foundation/P0094.md | Supplemental constraint only; merge target CODEX-0187 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0235-01-FOUNDATION-68c446a379 | P0095.md | supplemental-requirement | recorded only | shared_kernel::system_context_impl | codex-prompts/01-foundation/P0095.md | Supplemental constraint only; merge target CODEX-0190 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0236-01-FOUNDATION-76f8f9fc5c | P0096.md | supplemental-requirement | recorded only | shared_kernel::technology_selection_rust_impl | codex-prompts/01-foundation/P0096.md | Supplemental constraint only; merge target CODEX-0191 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0233-01-FOUNDATION-7a59670976 | P0097.md | supplemental-requirement | recorded only | shared_kernel::document_set_impl | codex-prompts/01-foundation/P0097.md | Supplemental constraint only; merge target CODEX-0188 if later executed. | Supplemental grep/coverage plus S01 checks. |
| CODEX-0234-01-FOUNDATION-dc1cae257b | P0098.md | supplemental-requirement | recorded only | shared_kernel::open_source_reference_matrix_impl | codex-prompts/01-foundation/P0098.md | Supplemental constraint only; merge target CODEX-0189 if later executed. | Supplemental grep/coverage plus S01 checks. |

## Execution Order

1. Read the mandatory governance inputs, S01 stage files, normalized maps, B006, and primary per-file prompt summaries.
2. Apply current-safe mapping for every referenced prompt before editing.
3. Preserve documentation-or-traceability Markdown outputs.
4. Implement only CODEX-0216 and CODEX-0222 Rust src/test outputs.
5. Keep all supplemental prompt rows recorded-only with no Rust output expansion.
6. Run target contract tests, S01 cargo checks, static boundary greps, and git diff whitespace checks.
7. Record evidence under evidence/batches/BATCH-006/.
