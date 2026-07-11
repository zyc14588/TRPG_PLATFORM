# BATCH-048 Work Plan

Batch: `BATCH-048-90-traceability`  
Stage: `S00 — governance onboarding`  
Prompt count: 25  
Primary prompts: 0  
Supplemental prompts: 0

## Scope

All rows are `documentation-or-traceability` / `traceability-maintenance`.
Allowed changes are the 25 current-safe Markdown traceability targets and
`evidence/batches/BATCH-048/` only. This batch does not own Rust `src/`,
product tests, migrations, API handlers, event schemas, NATS subjects,
metrics, workflows, provider adapters, or formal state-write paths.

Historical version labels, hashes, and old paths may appear only as provenance.
Executable naming and target ownership come from the current normalized and
safe maps.

## Prompt Map

| Prompt ID | Prompt file | Current-safe module | Current-safe target | Allowed change range | Test responsibility |
|---|---|---|---|---|---|
| `CODEX-1015-90-TRACEABILITY-7220c04846` | `codex-prompts/90-traceability/P0048.md` | `traceability::chatgpt_followup_research_prompts_impl` | `docs/codex/90-traceability/chatgpt_followup_research_prompts_impl.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1016-90-TRACEABILITY-fb8671acf0` | `codex-prompts/90-traceability/P0049.md` | `traceability::backlog_open_questions_impl` | `docs/codex/90-traceability/backlog_open_questions_impl.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1017-90-TRACEABILITY-6780a72e42` | `codex-prompts/90-traceability/P0050.md` | `traceability::implementation_plan_impl` | `docs/codex/90-traceability/implementation_plan_impl.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1018-90-TRACEABILITY-17a69c1cec` | `codex-prompts/90-traceability/P0051.md` | `traceability::source_implementation_completion_matrix_previousstrict` | `docs/codex/90-traceability/source_implementation_completion_matrix_previous-provenance.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1019-90-TRACEABILITY-92b0e07b7a` | `codex-prompts/90-traceability/P0055.md` | `traceability::source_processing_record_docs_implementation_00_index_crate_to_doc_map` | `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_crate_to_doc_map.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1020-90-TRACEABILITY-910724dc09` | `codex-prompts/90-traceability/P0054.md` | `traceability::source_processing_record_docs_implementation_00_index_doc_to_contract_map` | `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_doc_to_contract_map.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1021-90-TRACEABILITY-743ce38b70` | `codex-prompts/90-traceability/P0053.md` | `traceability::source_processing_record_docs_implementation_00_index_implementation_map` | `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_implementation_map.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1022-90-TRACEABILITY-96d3673432` | `codex-prompts/90-traceability/P0056.md` | `traceability::source_processing_record_docs_implementation_00_index_module_boundary_map` | `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_module_boundary_map.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1023-90-TRACEABILITY-ea4fa75543` | `codex-prompts/90-traceability/P0059.md` | `traceability::source_processing_record_docs_implementation_00_index_historical_to_current_mapping` | `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_old_to_new_mapping.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1024-90-TRACEABILITY-d824d01fa3` | `codex-prompts/90-traceability/P0058.md` | `traceability::source_processing_record_docs_implementation_00_index_reading_path` | `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_reading_path.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1025-90-TRACEABILITY-464adaf897` | `codex-prompts/90-traceability/P0052.md` | `traceability::source_processing_record_docs_implementation_00_index_readme` | `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_readme.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1026-90-TRACEABILITY-9e591ab06c` | `codex-prompts/90-traceability/P0057.md` | `traceability::source_processing_record_docs_implementation_00_index_reorganization_plan` | `docs/codex/90-traceability/source_processing_record_docs_implementation_00_index_reorganization_plan.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1027-90-TRACEABILITY-cbe6adc163` | `codex-prompts/90-traceability/P0066.md` | `traceability::source_processing_record_docs_implementation_90_traceability_adr_trace` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_adr_trace.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1028-90-TRACEABILITY-e32b9a2637` | `codex-prompts/90-traceability/P0067.md` | `traceability::source_processing_record_docs_implementation_90_traceability_completion_matrix` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_completion_matrix.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1029-90-TRACEABILITY-61ffcc7818` | `codex-prompts/90-traceability/P0064.md` | `traceability::source_processing_record_docs_implementation_90_traceability_historical_to_current_mapping` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_old_to_new_mapping.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1030-90-TRACEABILITY-39f2678c8b` | `codex-prompts/90-traceability/P0062.md` | `traceability::source_processing_record_docs_implementation_90_traceability_original_31_error_codes_metrics` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_original_31_error_codes_metrics.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1031-90-TRACEABILITY-b488aeb38c` | `codex-prompts/90-traceability/P0065.md` | `traceability::source_processing_record_docs_implementation_90_traceability_original_implementation_readme` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_original_implementation_readme.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1032-90-TRACEABILITY-18b3087a74` | `codex-prompts/90-traceability/P0060.md` | `traceability::source_processing_record_docs_implementation_90_traceability_readme` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_readme.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1033-90-TRACEABILITY-472c9c6961` | `codex-prompts/90-traceability/P0063.md` | `traceability::source_processing_record_docs_implementation_90_traceability_source_breakdown_roadmap_backlog_open_questions` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_source_breakdown_roadmap_backlog_open_questions.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1034-90-TRACEABILITY-fa75025ff7` | `codex-prompts/90-traceability/P0068.md` | `traceability::source_processing_record_docs_implementation_90_traceability_source_breakdown_roadmap_implementation_plan` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_source_breakdown_roadmap_implementation_plan.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1035-90-TRACEABILITY-2c814bb596` | `codex-prompts/90-traceability/P0061.md` | `traceability::source_processing_record_docs_implementation_readme` | `docs/codex/90-traceability/source_processing_record_docs_implementation_readme.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1036-90-TRACEABILITY-da4f4ec7a3` | `codex-prompts/90-traceability/P0070.md` | `traceability::source_processing_record_docs_roadmap_backlog_open_questions` | `docs/codex/90-traceability/source_processing_record_docs_roadmap_backlog_open_questions.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1037-90-TRACEABILITY-19fb2d9d0a` | `codex-prompts/90-traceability/P0069.md` | `traceability::source_processing_record_docs_roadmap_implementation_plan` | `docs/codex/90-traceability/source_processing_record_docs_roadmap_implementation_plan.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1038-90-TRACEABILITY-edeb3e648d` | `codex-prompts/90-traceability/P0071.md` | `traceability::source_processing_record_sources_coc_ai_trpg_top_level_design` | `docs/codex/90-traceability/source_processing_record_sources_coc_ai_trpg_top_level_design.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |
| `CODEX-1039-90-TRACEABILITY-7f61a8d14b` | `codex-prompts/90-traceability/P0074.md` | `traceability::source_processing_record_docs_implementation_90_traceability_source_breakdown_prompts_chatgpt_followup_research_prompts` | `docs/codex/90-traceability/source_processing_record_docs_implementation_90_traceability_source_breakdown_prompts_chatgpt_followup_research_prompts.md` | Markdown traceability/provenance only | Target exists; Prompt ID, module, source path/SHA, map agreement, docs-only boundary |

## Implementation Strategy

- Create one current-safe Markdown trace page per prompt.
- Record Prompt ID, prompt path, source path/SHA, current crate/module/output,
  role, and docs-only disposition.
- Do not copy historical Rust, SQL, API, event, NATS, metric, workflow, or test
  proposals from prompt provenance into current implementation.
- Keep pre-existing BATCH-047 working-tree changes untouched.

## Checks

Minimum relevant checks:

- Verify all 25 B048 current-safe targets exist.
- Verify every target contains its Prompt ID, current-safe module, source path,
  and source SHA.
- Verify normalized and safe maps agree for all 25 rows.
- Verify the B048 changed-file manifest contains exactly 25 docs plus 7 batch
  evidence files.
- Verify no B048 path crosses the documentation-only boundary.

S00 stage checks:

- Run `scripts/verify-governance-boundary.ps1`.
- Run available Cargo workspace format/check/tests because the workspace exists.
- Run the targeted visibility leakage test.
- Run root `pnpm.cmd test` as supplemental evidence only; it is not an S00 gate.
- Record Docker as not applicable because B048 changes no compose/container
  surface; Docker deployment belongs to S09/S13.
- Run `git diff --check`.

