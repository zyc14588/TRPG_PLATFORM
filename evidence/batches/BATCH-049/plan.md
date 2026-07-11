# BATCH-049 Work Plan

Batch: `BATCH-049-90-traceability — Strict Governance Final`  
Stage: `S00 — governance onboarding`  
Prompt count: 25  
Primary prompts: 0  
Supplemental prompts: 0  
Unique current-safe targets: 20

## Scope

All rows are `documentation-or-traceability` / `traceability-maintenance`.
Allowed changes are the 20 mapped Markdown targets and
`evidence/batches/BATCH-049/` only. Thirteen targets are missing and will be
created; seven existing canonical targets will receive additive B049 trace
sections. This batch does not own Rust `src/`, product tests, migrations, API
handlers, event schemas, NATS subjects, metrics, workflows, provider adapters,
or formal state-write paths.

Historical version labels, hashes, and old paths may appear only as
provenance. Current ownership comes from
`CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md` and
`CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`. Shared outputs are updated once and
retain every mapped Prompt ID; they are not replaced by prompt-specific files.

## Prompt Map

| Prompt ID | Prompt file | Current-safe module | Current-safe target | Allowed change range | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-1040-90-TRACEABILITY-d2d5dcf423` | `P0078.md` | `traceability::source_processing_record_docs_implementation_99_appendix_document_template` | `docs/codex/90-traceability/source_processing_record_docs_implementation_99_appendix_document_template.md` | Markdown trace/provenance only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1041-90-TRACEABILITY-25b31122f7` | `P0072.md` | `traceability::source_processing_record_docs_implementation_99_appendix_followup_research_prompts` | `docs/codex/90-traceability/source_processing_record_docs_implementation_99_appendix_followup_research_prompts.md` | Markdown trace/provenance only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1042-90-TRACEABILITY-b25f07d30c` | `P0077.md` | `traceability::source_processing_record_docs_implementation_99_appendix_glossary` | `docs/codex/90-traceability/source_processing_record_docs_implementation_99_appendix_glossary.md` | Markdown trace/provenance only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1043-90-TRACEABILITY-9d470d0b7c` | `P0076.md` | `traceability::source_processing_record_docs_implementation_99_appendix_implementation_doc_template` | `docs/codex/90-traceability/source_processing_record_docs_implementation_99_appendix_implementation_doc_template.md` | Markdown trace/provenance only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1044-90-TRACEABILITY-6b38bdeff4` | `P0073.md` | `traceability::source_processing_record_docs_implementation_99_appendix_open_source_reference_notes` | `docs/codex/90-traceability/source_processing_record_docs_implementation_99_appendix_open_source_reference_notes.md` | Markdown trace/provenance only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1045-90-TRACEABILITY-9a9028f88f` | `P0075.md` | `traceability::source_processing_record_docs_implementation_99_appendix_readme` | `docs/codex/90-traceability/source_processing_record_docs_implementation_99_appendix_readme.md` | Markdown trace/provenance only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1046-90-TRACEABILITY-e1eaaefb1a` | `P0079.md` | `traceability::source_processing_record_docs_implementation_99_appendix_unresolved_questions` | `docs/codex/90-traceability/source_processing_record_docs_implementation_99_appendix_unresolved_questions.md` | Markdown trace/provenance only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1047-90-TRACEABILITY-9adca5662e` | `P0080.md` | `traceability::source_processing_record_docs_prompts_chatgpt_followup_research_prompts` | `docs/codex/90-traceability/source_processing_record_docs_prompts_chatgpt_followup_research_prompts.md` | Markdown trace/provenance only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1048-90-TRACEABILITY-93cb79e8d2` | `P0081.md` | `traceability::source_processing_index` | `docs/codex/90-traceability/source_processing_index.md` | Markdown index/trace only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1049-90-TRACEABILITY-ced604b3cf` | `P0082.md` | `traceability::strict_source_disposition_matrix` | `docs/codex/90-traceability/strict_source_disposition_matrix.md` | Markdown matrix/trace only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1050-90-TRACEABILITY-67efd526d9` | `P0083.md` | `traceability::template_debt_remediation` | `docs/codex/90-traceability/template_debt_remediation.md` | Markdown remediation/trace only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1051-90-TRACEABILITY-5058f071c1` | `P0084.md` | `traceability::chatgpt_followup_research_prompts` | `docs/codex/90-traceability/chatgpt_followup_research_prompts.md` | Additive Markdown trace only | Preserve prior rows; verify new ID, module, source/SHA, docs-only boundary |
| `CODEX-1052-90-TRACEABILITY-abac7952b1` | `P0085.md` | `traceability::docs_implementation_99_appendix_readme_previous_provenance` | `docs/codex/90-traceability/docs_implementation_99_appendix_readme_strict_previous.md` | Provenance Markdown only | Exact normalized override, ID, module, source/SHA, non-current acceptance boundary |
| `CODEX-1053-90-TRACEABILITY-91cc9c1979` | `P0086.md` | `traceability::readme` | `docs/codex/90-traceability/readme.md` | Shared additive Markdown trace | Preserve prior content; verify all five B049 readme rows |
| `CODEX-1054-90-TRACEABILITY-74f2cf5e3b` | `P0087.md` | `traceability::readme` | `docs/codex/90-traceability/readme.md` | Shared additive Markdown trace | Preserve prior content; verify all five B049 readme rows |
| `CODEX-1055-90-TRACEABILITY-964c983038` | `P0088.md` | `traceability::manifest` | `docs/codex/90-traceability/manifest.md` | Markdown manifest/trace only | Target, ID, module, prompt/source path and SHA, map agreement, docs-only boundary |
| `CODEX-1056-90-TRACEABILITY-cc337fd4d2` | `P0089.md` | `traceability::readme` | `docs/codex/90-traceability/readme.md` | Shared additive Markdown trace | Preserve prior content; verify all five B049 readme rows |
| `CODEX-1057-90-TRACEABILITY-d04d7696e9` | `P0090.md` | `traceability::historical_to_current_mapping` | `docs/codex/90-traceability/old_to_new_mapping.md` | Shared additive provenance trace | Preserve prior rows; verify normalized target override and both B049 rows |
| `CODEX-1058-90-TRACEABILITY-d3a03a9d63` | `P0091.md` | `traceability::readme` | `docs/codex/90-traceability/readme.md` | Shared additive Markdown trace | Preserve prior content; verify all five B049 readme rows |
| `CODEX-1059-90-TRACEABILITY-1427a2ad0e` | `P0092.md` | `traceability::adr_trace` | `docs/codex/90-traceability/adr_trace.md` | Additive Markdown trace only | Preserve prior rows; verify new ID, module, source/SHA, docs-only boundary |
| `CODEX-1060-90-TRACEABILITY-c30586dd61` | `P0093.md` | `traceability::completion_matrix` | `docs/codex/90-traceability/completion_matrix.md` | Additive Markdown matrix trace | Preserve prior rows; verify new ID, module, source/SHA, docs-only boundary |
| `CODEX-1061-90-TRACEABILITY-5ef9be1888` | `P0094.md` | `traceability::historical_to_current_mapping` | `docs/codex/90-traceability/old_to_new_mapping.md` | Shared additive provenance trace | Preserve prior rows; verify normalized target override and both B049 rows |
| `CODEX-1062-90-TRACEABILITY-d2f07cf2be` | `P0096.md` | `traceability::original_31_error_codes_metrics` | `docs/codex/90-traceability/original_31_error_codes_metrics.md` | Additive provenance trace only | Preserve prior row; verify new ID/module/source/SHA and no executable metric |
| `CODEX-1063-90-TRACEABILITY-004e350cea` | `P0095.md` | `traceability::original_implementation_readme` | `docs/codex/90-traceability/original_implementation_readme.md` | Additive provenance trace only | Preserve prior row; verify new ID/module/source/SHA and docs-only boundary |
| `CODEX-1064-90-TRACEABILITY-20708447f6` | `P0097.md` | `traceability::readme` | `docs/codex/90-traceability/readme.md` | Shared additive Markdown trace | Preserve prior content; verify all five B049 readme rows |

## Implementation Strategy

- Reuse the established B046-B048 trace-page format.
- Create one page for each missing unique current-safe target.
- Append one marked B049 section to each existing canonical target; combine the
  five `readme.md` rows and two `old_to_new_mapping.md` rows.
- Record Prompt ID, prompt path, source path/SHA, current crate/module/output,
  role, and docs-only disposition.
- Do not copy historical code, SQL, API, event, NATS, metric, workflow, or test
  proposals into current implementation.

## Checks

Minimum B049 checks:

- Parse all 25 batch rows and prompts; require 20 unique mapped targets.
- Require normalized/safe map agreement for every row.
- Require each target to contain its Prompt ID, current-safe module, prompt
  path, source path, and source SHA.
- Validate Markdown H1, balanced fences, and table column counts.
- Require the final B049 manifest to contain exactly 20 docs plus seven
  evidence files, with no product implementation paths.
- Scan B049-owned paths for executable provider calls, direct formal-state
  writes, and sensitive-label fixture leakage.

S00 stage checks:

- Run `scripts/verify-governance-boundary.ps1`.
- Run Cargo format, check, workspace tests, and targeted visibility leakage.
- Run root `pnpm.cmd test` as supplemental workspace evidence.
- Record Docker as not applicable because B049 changes no compose/container
  surface; Docker deployment belongs to S09/S13.
- Run `git diff --check`.
