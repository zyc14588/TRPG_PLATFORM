# BATCH-001 Work Plan

Batch: `BATCH-001-00-index — Strict Governance Final`
Stage: `S00 — governance onboarding`
Prompt count: 25
Primary prompt count: 0
Allowed scope: documentation and traceability only

## Read Inputs

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `batches/B001.md`
- all 25 B001 per-file prompts
- S00 README, START, TEST_PLAN, TEST_DATA, ACCEPTANCE, and REPAIR prompts

## Plan

| Prompt ID | Current-safe target | Allowed change | Test responsibility |
|---|---|---|---|
| CODEX-0001-00-INDEX-996d963665 | `docs/codex/00-index/manifest.md` | create docs-governance manifest note | prompt count, role, target closure |
| CODEX-0002-00-INDEX-8ec9a00bfd | `docs/codex/00-index/readme.md` | update current-safe readme coverage note | target resolves, docs-only scope |
| CODEX-0003-00-INDEX-bc3fa6f721 | `docs/codex/00-index/previous-delivery-report-provenance.md` | create provenance note | previous report not current acceptance |
| CODEX-0004-00-INDEX-7216d1d127 | `docs/codex/00-index/validation.md` | create validation boundary | batch validation rules present |
| CODEX-0005-00-INDEX-be84920579 | `docs/codex/00-index/m_00_index.md` | create module overview | module scope remains docs-only |
| CODEX-0006-00-INDEX-9ddd6d3ff2 | `docs/codex/00-index/canonical_document_boundary_previous.md` | create previous provenance boundary | no historical current names |
| CODEX-0007-00-INDEX-60bd308841 | `docs/codex/00-index/crate_to_doc_map.md` | create shared crate map | shared target closure |
| CODEX-0008-00-INDEX-337c7efeb8 | `docs/codex/00-index/decision_trace_map.md` | create decision trace map | authority source list present |
| CODEX-0009-00-INDEX-ec6ba70aa0 | `docs/codex/00-index/doc_to_contract_map.md` | create shared doc-contract map | contract boundary present |
| CODEX-0010-00-INDEX-68fb192697 | `docs/codex/00-index/implementation_map.md` | create implementation scope map | primary count is zero |
| CODEX-0011-00-INDEX-b7086d0435 | `docs/codex/00-index/historical_cleanup_policy.md` | create cleanup policy | historical tokens provenance only |
| CODEX-0012-00-INDEX-9f38048f59 | `docs/codex/00-index/module_boundary_map.md` | create module boundary map | no Rust/API/event/NATS outputs |
| CODEX-0013-00-INDEX-42bafcf994 | `docs/codex/00-index/reading_path.md` | create reading path | normalized maps precede prompts |
| CODEX-0014-00-INDEX-fb679a84d1 | `docs/codex/00-index/source_to_code_ready_map.md` | create source-to-output rule | source snippets not executable |
| CODEX-0115-00-INDEX-907158d7fb | `docs/codex/00-index/canonical_document_boundary_previous-provenance.md` | create previous strict provenance | current-safe rewrite documented |
| CODEX-0116-00-INDEX-5fe281d240 | `docs/codex/00-index/contract_index.md` | create contract index | top-level red lines preserved |
| CODEX-0117-00-INDEX-a7ff60b697 | `docs/codex/00-index/crate_to_doc_map.md` | converge on shared crate map | no duplicate Rust owner |
| CODEX-0118-00-INDEX-215c4c75fb | `docs/codex/00-index/doc_to_contract_map.md` | converge on shared doc-contract map | no duplicate Rust owner |
| CODEX-0119-00-INDEX-f7d38e1298 | `docs/codex/00-index/docs_implementation.md` | create docs implementation note | code snippets remain provenance |
| CODEX-0120-00-INDEX-34b342f96e | `docs/codex/00-index/adr_0001_rust_first.md` | create ADR governance note | no Rust output in B001 |
| CODEX-0121-00-INDEX-9d76fa3212 | `docs/codex/00-index/crate.md` | create crate governance note | crate name is docs owner only |
| CODEX-0122-00-INDEX-add3d8f14b | `docs/codex/00-index/docs_implementation_00_index_doc_to_contract_map_strict_previous.md` | create previous provenance note | no source-path-derived names |
| CODEX-0123-00-INDEX-e1287f379d | `docs/codex/00-index/docs_implementation_00_index_implementation_map_strict_previous.md` | create previous provenance note | no source-path-derived names |
| CODEX-0124-00-INDEX-abf101aa06 | `docs/codex/00-index/docs_implementation_00_index_module_boundary_map_strict_previous.md` | create previous provenance note | no source-path-derived names |
| CODEX-0125-00-INDEX-b9c587ede5 | `docs/codex/00-index/docs_implementation_00_index_reading_path_strict_previous.md` | create previous provenance note | no source-path-derived names |

## Not In Scope

- No product Rust code.
- No tests under `tests/` because no primary implementation exists and no Cargo workspace exists in this checkout.
- No migrations, API handlers, event schemas, NATS subjects, workflows, metrics, or provider integrations.
