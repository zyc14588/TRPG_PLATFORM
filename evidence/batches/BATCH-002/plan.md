# BATCH-002 Work Plan

Batch: `BATCH-002-00-index`
Current batch file: `batches/B002.md`

## Scope

- Declared prompt count: 23.
- Primary prompt count: 0.
- Supplemental prompt count: 0.
- All rows are `docs-governance` with `documentation-or-traceability` output.
- Allowed changes: Markdown governance, 00-index traceability, current-safe
  mapping evidence, and Batch-002 acceptance evidence.
- Disallowed changes: Rust source, runtime tests, migrations, handlers, event
  schemas, NATS subjects, workflows, metrics, provider adapters, Authority
  Contract behavior, visibility behavior, dice behavior, or model routing
  behavior.

## Prompt Mapping

| Prompt ID | Prompt file | Target file | Allowed evidence scope | Test responsibility |
|---|---|---|---|---|
| CODEX-0126-00-INDEX-5196c3a177 | `codex-prompts/00-index/P0017.md` | `docs/codex/00-index/docs_implementation.md` | Docs-only governance trace | Check current-safe target and docs-only boundary |
| CODEX-0127-00-INDEX-f1b2fff17d | `codex-prompts/00-index/P0023.md` | `docs/codex/00-index/coc_ai_kp.md` | AI KP governance trace | Check no provider/runtime implementation |
| CODEX-0128-00-INDEX-029b644688 | `codex-prompts/00-index/P0024.md` | `docs/codex/00-index/generated_output_map.md` | Generated output map trace | Check all listed outputs are Markdown/evidence |
| CODEX-0129-00-INDEX-f22cda391d | `codex-prompts/00-index/P0025.md` | `docs/codex/00-index/implementation_map.md` | Implementation boundary trace | Check primary prompt count remains 0 |
| CODEX-0130-00-INDEX-f9fc3b2eea | `codex-prompts/00-index/P0026.md` | `docs/codex/00-index/module_boundary_map.md` | Module boundary trace | Check no implementation paths are authorized |
| CODEX-0131-00-INDEX-e0a1e1fe53 | `codex-prompts/00-index/P0027.md` | `docs/codex/00-index/processing_summary.md` | Processing summary trace | Check batch path normalization is recorded |
| CODEX-0132-00-INDEX-906a5df715 | `codex-prompts/00-index/P0028.md` | `docs/codex/00-index/reading_path.md` | Reading path trace | Check required map-reading order is preserved |
| CODEX-0133-00-INDEX-07ca4e0897 | `codex-prompts/00-index/P0029.md` | `docs/codex/00-index/recommended_tree_landing_report_previous-provenance.md` | Provenance-only landing page trace | Check historical names remain provenance |
| CODEX-0134-00-INDEX-6f0a6dfd3d | `codex-prompts/00-index/P0030.md` | `docs/codex/00-index/reorganization_plan.md` | Docs-only reorganization trace | Check no file moves outside scope |
| CODEX-0135-00-INDEX-12ca8e24ff | `codex-prompts/00-index/P0031.md` | `docs/codex/00-index/source_to_output_strict_map.md` | Strict source-to-output trace | Check current-safe output names are used |
| CODEX-0136-00-INDEX-8cbf79c9f0 | `codex-prompts/00-index/P0032.md` | `docs/codex/00-index/implementation_acceptance_checklist.md` | Acceptance checklist trace | Check no acceptance weakening |
| CODEX-0137-00-INDEX-043d2e276c | `codex-prompts/00-index/P0033.md` | `docs/codex/00-index/crate_to_doc_map.md` | Crate-to-doc trace | Check no Rust crate is created |
| CODEX-0138-00-INDEX-998644def6 | `codex-prompts/00-index/P0034.md` | `docs/codex/00-index/doc_to_contract_map.md` | Contract coverage trace | Check top-level red lines are preserved |
| CODEX-0139-00-INDEX-524f2c1e4c | `codex-prompts/00-index/P0035.md` | `docs/codex/00-index/implementation_map.md` | Shared implementation boundary trace | Check shared target has no duplicate owner conflict |
| CODEX-0140-00-INDEX-74bdec684b | `codex-prompts/00-index/P0036.md` | `docs/codex/00-index/module_boundary_map.md` | Shared module boundary trace | Check docs_governance remains docs-only |
| CODEX-0141-00-INDEX-9f34d949a7 | `codex-prompts/00-index/P0037.md` | `docs/codex/00-index/reading_path.md` | Shared reading path trace | Check normalized maps are mandatory |
| CODEX-0142-00-INDEX-cda051418c | `codex-prompts/00-index/P0038.md` | `docs/codex/00-index/reorganization_plan.md` | Shared reorganization trace | Check no scope expansion |
| CODEX-0143-00-INDEX-1a6e90c5e3 | `codex-prompts/00-index/P0039.md` | `docs/codex/00-index/backlog_open_questions.md` | Backlog questions trace | Check open items do not authorize code |
| CODEX-0144-00-INDEX-0e145fe266 | `codex-prompts/00-index/P0040.md` | `docs/codex/00-index/implementation_plan.md` | No-primary implementation plan trace | Check plan remains future-facing only |
| CODEX-0145-00-INDEX-b7a82ff149 | `codex-prompts/00-index/P0041.md` | `docs/codex/00-index/coc_ai_trpg_top_level_design.md` | Top-level design trace | Check design red lines are preserved |
| CODEX-0146-00-INDEX-41af9b82fe | `codex-prompts/00-index/P0042.md` | `docs/codex/00-index/strict_rework_report.md` | Strict rework trace | Check only docs/evidence were reworked |
| CODEX-1108-00-INDEX-e7e466381b | `codex-prompts/00-index/P0043.md` | `docs/codex/00-index/readme.md` | 00-index readme trace | Check index remains docs-only |
| CODEX-1109-00-INDEX-4e42f0301d | `codex-prompts/00-index/P0045.md` | `docs/codex/00-index/previous-audit-provenance_json.md` | Previous audit provenance trace | Check historical JSON name is not promoted |

## Check Order

1. Minimal Batch-002 current-safe assertions.
2. Stage S00 active strict checks, using inline validation evidence because the
   legacy Python helpers are optional local helpers in this checkout.
3. Fixture presence and parse checks.
4. Acceptance evidence write-up under `evidence/batches/BATCH-002/`.

## Expected Strict Result

`PASS` when the inline S00 strict checks pass. Cargo is not applicable for this
docs-only batch while no `Cargo.toml` exists and Batch-002 has 0 product-code
prompts.
