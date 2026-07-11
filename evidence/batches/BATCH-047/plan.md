# BATCH-047 Work Plan

Batch: `BATCH-047-90-traceability`
Stage: `S00 — governance onboarding`
Prompt count: 25
Primary prompts: 0

## Scope

All rows are `documentation-or-traceability` / `traceability-maintenance`.
Allowed changes are Markdown traceability outputs and batch evidence only.
This batch must not create or modify Rust `src/`, product tests, migrations,
API handlers, event schemas, NATS subjects, metrics, workflow code, provider
adapters, or formal state-write paths.

## Prompt Map

| Prompt ID | Prompt file | Current-safe target | Allowed change range | Test responsibility |
|---|---|---|---|---|
| `CODEX-0990-90-TRACEABILITY-a87cf26263` | `codex-prompts/90-traceability/P0036.md` | `docs/codex/90-traceability/docs_implementation_90_traceability_adr_trace_strict_previous.md` | Markdown provenance trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0991-90-TRACEABILITY-e765975144` | `codex-prompts/90-traceability/P0042.md` | `docs/codex/90-traceability/docs_implementation_90_traceability_completion_matrix_strict_previous.md` | Markdown provenance trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0992-90-TRACEABILITY-c348634108` | `codex-prompts/90-traceability/P0039.md` | `docs/codex/90-traceability/docs_implementation_90_traceability_old_to_new_mapping_strict_previous.md` | Markdown provenance trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0993-90-TRACEABILITY-bdf96750ce` | `codex-prompts/90-traceability/P0038.md` | `docs/codex/90-traceability/docs_implementation_90_traceability_original_implementation_read.md` | Append Markdown trace to existing doc | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0994-90-TRACEABILITY-734ecdf4a7` | `codex-prompts/90-traceability/P0032.md` | `docs/codex/90-traceability/docs_implementation_90_traceability_original_31_error_codes_metr.md` | Append Markdown trace to existing doc | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0995-90-TRACEABILITY-f38d6fb6a8` | `codex-prompts/90-traceability/P0043.md` | `docs/codex/90-traceability/docs_implementation_90_traceability_readme_strict_previous.md` | Markdown provenance trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0996-90-TRACEABILITY-6e66ea648b` | `codex-prompts/90-traceability/P0031.md` | `docs/codex/90-traceability/system_context.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0997-90-TRACEABILITY-e68dd8440b` | `codex-prompts/90-traceability/P0041.md` | `docs/codex/90-traceability/cargo_workspace.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0998-90-TRACEABILITY-368f0cb4e9` | `codex-prompts/90-traceability/P0028.md` | `docs/codex/90-traceability/backlog_open_questions.md` | Append Markdown trace to existing doc | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-0999-90-TRACEABILITY-785cfbf52b` | `codex-prompts/90-traceability/P0033.md` | `docs/codex/90-traceability/document_set.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1000-90-TRACEABILITY-10b80d60f4` | `codex-prompts/90-traceability/P0025.md` | `docs/codex/90-traceability/memory_rag.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1001-90-TRACEABILITY-8df7868152` | `codex-prompts/90-traceability/P0034.md` | `docs/codex/90-traceability/source_breakdown_architecture_technology_selection_rust.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1002-90-TRACEABILITY-4de2f4b503` | `codex-prompts/90-traceability/P0029.md` | `docs/codex/90-traceability/implementation_plan.md` | Append Markdown trace to existing doc | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1003-90-TRACEABILITY-18e59dc953` | `codex-prompts/90-traceability/P0027.md` | `docs/codex/90-traceability/domain_model.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1004-90-TRACEABILITY-0492b9deab` | `codex-prompts/90-traceability/P0023.md` | `docs/codex/90-traceability/investigation_clue_npc_time.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1005-90-TRACEABILITY-da1bf6bd16` | `codex-prompts/90-traceability/P0040.md` | `docs/codex/90-traceability/chatgpt_followup_research_prompts.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1006-90-TRACEABILITY-972996e232` | `codex-prompts/90-traceability/P0035.md` | `docs/codex/90-traceability/session_runtime.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1007-90-TRACEABILITY-13bd257921` | `codex-prompts/90-traceability/P0026.md` | `docs/codex/90-traceability/open_source_reference_matrix.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1008-90-TRACEABILITY-08607f8d2f` | `codex-prompts/90-traceability/P0024.md` | `docs/codex/90-traceability/source_breakdown_domain_command_cqrs.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1009-90-TRACEABILITY-b2ca11d280` | `codex-prompts/90-traceability/P0037.md` | `docs/codex/90-traceability/api_contracts.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1010-90-TRACEABILITY-6285bea0b6` | `codex-prompts/90-traceability/P0030.md` | `docs/codex/90-traceability/model_provider_local_cloud.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1011-90-TRACEABILITY-32e59e6840` | `codex-prompts/90-traceability/P0044.md` | `docs/codex/90-traceability/old_to_new_mapping.md` | Append Markdown trace to existing doc | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1012-90-TRACEABILITY-9a9068cc0b` | `codex-prompts/90-traceability/P0045.md` | `docs/codex/90-traceability/original_31_error_codes_metrics.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1013-90-TRACEABILITY-73f17bc951` | `codex-prompts/90-traceability/P0046.md` | `docs/codex/90-traceability/original_implementation_readme.md` | Markdown trace only | Output exists, Prompt ID present, docs-only boundary |
| `CODEX-1014-90-TRACEABILITY-3e2e7e413e` | `codex-prompts/90-traceability/P0047.md` | `docs/codex/90-traceability/readme.md` | Append Markdown trace to existing readme | Output exists, Prompt ID present, docs-only boundary |

## Checks

Minimum relevant checks:

- Verify 25 current-safe target files exist.
- Verify each target contains its B047 Prompt ID.
- Verify no B047 change touches Rust `src/`, product tests, migrations, API,
  event, NATS, metric, workflow, provider adapter, or formal write paths.

S00 docs-only stage checks:

- Verify root governance files and S00 boundary files exist.
- Verify batch count remains 52 and B047 remains docs-only.
- Run `scripts/verify-governance-boundary.ps1` as the authorized S00
  non-product fixture assertion.
- Run Cargo workspace sanity because a workspace now exists.
- Run the available pnpm test as supplemental workspace evidence; it belongs
  to S12 and is not a B047 gate.
- Record Docker as not applicable because B047/S00 changes no compose or
  container surface; Docker deployment is owned by S09/S13.
- Keep the B047 manifest at 32 paths (25 docs + 7 evidence) and report the
  authorized S00 repair script separately, for 33 combined relevant paths.
