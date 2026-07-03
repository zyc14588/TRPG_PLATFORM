> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/00-index/per-file-prompt-manifest.md`
> 筛选状态：`active-index`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# 00-index — Per-file Prompt Manifest（Strict Governance Final）

> Prompt 数量：48。第五次严格修复后，supplemental prompt 只补需求，primary prompt 才能拥有 Rust 输出文件。

| # | Prompt ID | 任务类型 | 输出角色 | Primary Prompt | module | 建议输出文件 | Prompt 文件 |
|---:|---|---|---|---|---|---|---|
| 1 | CODEX-0001-00-INDEX-996d963665 | docs-governance | documentation-or-traceability |  | docs_governance::manifest | `docs/codex/00-index/manifest.md` | codex-prompts/00-index/P0044.md |
| 2 | CODEX-0002-00-INDEX-8ec9a00bfd | docs-governance | documentation-or-traceability |  | docs_governance::readme | `docs/codex/00-index/readme.md` | codex-prompts/00-index/P0047.md |
| 3 | CODEX-0003-00-INDEX-bc3fa6f721 | docs-governance | documentation-or-traceability |  | docs_governance::previous_delivery_report_provenance | `docs/codex/00-index/previous_delivery_report_provenance.md` | codex-prompts/00-index/P0046.md |
| 4 | CODEX-0004-00-INDEX-7216d1d127 | docs-governance | documentation-or-traceability |  | docs_governance::validation | `docs/codex/00-index/validation.md` | codex-prompts/00-index/P0048.md |
| 5 | CODEX-0005-00-INDEX-be84920579 | docs-governance | documentation-or-traceability |  | docs_governance::m_00_index | `docs/codex/00-index/m_00_index.md` | codex-prompts/00-index/P0009.md |
| 6 | CODEX-0006-00-INDEX-9ddd6d3ff2 | docs-governance | documentation-or-traceability |  | docs_governance::canonical_document_boundary | `docs/codex/00-index/canonical_document_boundary.md` | codex-prompts/00-index/P0001.md |
| 7 | CODEX-0007-00-INDEX-60bd308841 | docs-governance | documentation-or-traceability |  | docs_governance::crate_to_doc_map | `docs/codex/00-index/crate_to_doc_map.md` | codex-prompts/00-index/P0002.md |
| 8 | CODEX-0008-00-INDEX-337c7efeb8 | docs-governance | documentation-or-traceability |  | docs_governance::decision_trace_map | `docs/codex/00-index/decision_trace_map.md` | codex-prompts/00-index/P0003.md |
| 9 | CODEX-0009-00-INDEX-ec6ba70aa0 | docs-governance | documentation-or-traceability |  | docs_governance::doc_to_contract_map | `docs/codex/00-index/doc_to_contract_map.md` | codex-prompts/00-index/P0004.md |
| 10 | CODEX-0010-00-INDEX-68fb192697 | docs-governance | documentation-or-traceability |  | docs_governance::implementation_map | `docs/codex/00-index/implementation_map.md` | codex-prompts/00-index/P0005.md |
| 11 | CODEX-0011-00-INDEX-b7086d0435 | docs-governance | documentation-or-traceability |  | docs_governance::historical_cleanup_policy | `docs/codex/00-index/historical_cleanup_policy.md` | codex-prompts/00-index/P0007.md |
| 12 | CODEX-0012-00-INDEX-9f38048f59 | docs-governance | documentation-or-traceability |  | docs_governance::module_boundary_map | `docs/codex/00-index/module_boundary_map.md` | codex-prompts/00-index/P0006.md |
| 13 | CODEX-0013-00-INDEX-42bafcf994 | docs-governance | documentation-or-traceability |  | docs_governance::reading_path | `docs/codex/00-index/reading_path.md` | codex-prompts/00-index/P0008.md |
| 14 | CODEX-0014-00-INDEX-fb679a84d1 | docs-governance | documentation-or-traceability |  | docs_governance::source_to_code_ready_map | `docs/codex/00-index/source_to_code_ready_map.md` | codex-prompts/00-index/P0010.md |
| 15 | CODEX-0115-00-INDEX-907158d7fb | docs-governance | documentation-or-traceability |  | docs_governance::canonical_document_boundary_strict | `docs/codex/00-index/canonical_document_boundary_strict.md` | codex-prompts/00-index/P0011.md |
| 16 | CODEX-0116-00-INDEX-5fe281d240 | docs-governance | documentation-or-traceability |  | docs_governance::contract_index | `docs/codex/00-index/contract_index.md` | codex-prompts/00-index/P0012.md |
| 17 | CODEX-0117-00-INDEX-a7ff60b697 | docs-governance | documentation-or-traceability |  | docs_governance::crate_to_doc_map | `docs/codex/00-index/crate_to_doc_map.md` | codex-prompts/00-index/P0013.md |
| 18 | CODEX-0118-00-INDEX-215c4c75fb | docs-governance | documentation-or-traceability |  | docs_governance::doc_to_contract_map | `docs/codex/00-index/doc_to_contract_map.md` | codex-prompts/00-index/P0014.md |
| 19 | CODEX-0119-00-INDEX-f7d38e1298 | docs-governance | documentation-or-traceability |  | docs_governance::docs_implementation | `docs/codex/00-index/docs_implementation.md` | codex-prompts/00-index/P0015.md |
| 20 | CODEX-0120-00-INDEX-34b342f96e | docs-governance | documentation-or-traceability |  | docs_governance::adr_0001_rust_first | `docs/codex/00-index/adr_0001_rust_first.md` | codex-prompts/00-index/P0016.md |
| 21 | CODEX-0121-00-INDEX-9d76fa3212 | docs-governance | documentation-or-traceability |  | docs_governance::crate | `docs/codex/00-index/crate.md` | codex-prompts/00-index/P0018.md |
| 22 | CODEX-0122-00-INDEX-add3d8f14b | docs-governance | documentation-or-traceability |  | docs_governance::docs_implementation_00_index_doc_to_contract_map_strict | `docs/codex/00-index/docs_implementation_00_index_doc_to_contract_map_strict.md` | codex-prompts/00-index/P0020.md |
| 23 | CODEX-0123-00-INDEX-e1287f379d | docs-governance | documentation-or-traceability |  | docs_governance::docs_implementation_00_index_implementation_map_strict | `docs/codex/00-index/docs_implementation_00_index_implementation_map_strict.md` | codex-prompts/00-index/P0022.md |
| 24 | CODEX-0124-00-INDEX-abf101aa06 | docs-governance | documentation-or-traceability |  | docs_governance::docs_implementation_00_index_module_boundary_map_strict | `docs/codex/00-index/docs_implementation_00_index_module_boundary_map_strict.md` | codex-prompts/00-index/P0019.md |
| 25 | CODEX-0125-00-INDEX-b9c587ede5 | docs-governance | documentation-or-traceability |  | docs_governance::docs_implementation_00_index_reading_path_strict | `docs/codex/00-index/docs_implementation_00_index_reading_path_strict.md` | codex-prompts/00-index/P0021.md |
| 26 | CODEX-0126-00-INDEX-5196c3a177 | docs-governance | documentation-or-traceability |  | docs_governance::docs_implementation | `docs/codex/00-index/docs_implementation.md` | codex-prompts/00-index/P0017.md |
| 27 | CODEX-0127-00-INDEX-f1b2fff17d | docs-governance | documentation-or-traceability |  | docs_governance::coc_ai_kp | `docs/codex/00-index/coc_ai_kp.md` | codex-prompts/00-index/P0023.md |
| 28 | CODEX-0128-00-INDEX-029b644688 | docs-governance | documentation-or-traceability |  | docs_governance::generated_output_map | `docs/codex/00-index/generated_output_map.md` | codex-prompts/00-index/P0024.md |
| 29 | CODEX-0129-00-INDEX-f22cda391d | docs-governance | documentation-or-traceability |  | docs_governance::implementation_map | `docs/codex/00-index/implementation_map.md` | codex-prompts/00-index/P0025.md |
| 30 | CODEX-0130-00-INDEX-f9fc3b2eea | docs-governance | documentation-or-traceability |  | docs_governance::module_boundary_map | `docs/codex/00-index/module_boundary_map.md` | codex-prompts/00-index/P0026.md |
| 31 | CODEX-0131-00-INDEX-e0a1e1fe53 | docs-governance | documentation-or-traceability |  | docs_governance::processing_summary | `docs/codex/00-index/processing_summary.md` | codex-prompts/00-index/P0027.md |
| 32 | CODEX-0132-00-INDEX-906a5df715 | docs-governance | documentation-or-traceability |  | docs_governance::reading_path | `docs/codex/00-index/reading_path.md` | codex-prompts/00-index/P0028.md |
| 33 | CODEX-0133-00-INDEX-07ca4e0897 | docs-governance | documentation-or-traceability |  | docs_governance::recommended_tree_landing_report_previous_strict | `docs/codex/00-index/recommended_tree_landing_report_previous_strict.md` | codex-prompts/00-index/P0029.md |
| 34 | CODEX-0134-00-INDEX-6f0a6dfd3d | docs-governance | documentation-or-traceability |  | docs_governance::reorganization_plan | `docs/codex/00-index/reorganization_plan.md` | codex-prompts/00-index/P0030.md |
| 35 | CODEX-0135-00-INDEX-12ca8e24ff | docs-governance | documentation-or-traceability |  | docs_governance::source_to_output_strict_map | `docs/codex/00-index/source_to_output_strict_map.md` | codex-prompts/00-index/P0031.md |
| 36 | CODEX-0136-00-INDEX-8cbf79c9f0 | docs-governance | documentation-or-traceability |  | docs_governance::implementation_acceptance_checklist | `docs/codex/00-index/implementation_acceptance_checklist.md` | codex-prompts/00-index/P0032.md |
| 37 | CODEX-0137-00-INDEX-043d2e276c | docs-governance | documentation-or-traceability |  | docs_governance::crate_to_doc_map | `docs/codex/00-index/crate_to_doc_map.md` | codex-prompts/00-index/P0033.md |
| 38 | CODEX-0138-00-INDEX-998644def6 | docs-governance | documentation-or-traceability |  | docs_governance::doc_to_contract_map | `docs/codex/00-index/doc_to_contract_map.md` | codex-prompts/00-index/P0034.md |
| 39 | CODEX-0139-00-INDEX-524f2c1e4c | docs-governance | documentation-or-traceability |  | docs_governance::implementation_map | `docs/codex/00-index/implementation_map.md` | codex-prompts/00-index/P0035.md |
| 40 | CODEX-0140-00-INDEX-74bdec684b | docs-governance | documentation-or-traceability |  | docs_governance::module_boundary_map | `docs/codex/00-index/module_boundary_map.md` | codex-prompts/00-index/P0036.md |
| 41 | CODEX-0141-00-INDEX-9f34d949a7 | docs-governance | documentation-or-traceability |  | docs_governance::reading_path | `docs/codex/00-index/reading_path.md` | codex-prompts/00-index/P0037.md |
| 42 | CODEX-0142-00-INDEX-cda051418c | docs-governance | documentation-or-traceability |  | docs_governance::reorganization_plan | `docs/codex/00-index/reorganization_plan.md` | codex-prompts/00-index/P0038.md |
| 43 | CODEX-0143-00-INDEX-1a6e90c5e3 | docs-governance | documentation-or-traceability |  | docs_governance::backlog_open_questions | `docs/codex/00-index/backlog_open_questions.md` | codex-prompts/00-index/P0039.md |
| 44 | CODEX-0144-00-INDEX-0e145fe266 | docs-governance | documentation-or-traceability |  | docs_governance::implementation_plan | `docs/codex/00-index/implementation_plan.md` | codex-prompts/00-index/P0040.md |
| 45 | CODEX-0145-00-INDEX-b7a82ff149 | docs-governance | documentation-or-traceability |  | docs_governance::coc_ai_trpg_top_level_design | `docs/codex/00-index/coc_ai_trpg_top_level_design.md` | codex-prompts/00-index/P0041.md |
| 46 | CODEX-0146-00-INDEX-41af9b82fe | docs-governance | documentation-or-traceability |  | docs_governance::strict_rework_report | `docs/codex/00-index/strict_rework_report.md` | codex-prompts/00-index/P0042.md |
| 47 | CODEX-1108-00-INDEX-e7e466381b | docs-governance | documentation-or-traceability |  | docs_governance::readme | `docs/codex/00-index/readme.md` | codex-prompts/00-index/P0043.md |
| 48 | CODEX-1109-00-INDEX-4e42f0301d | docs-governance | documentation-or-traceability |  | docs_governance::previous_audit_json_provenance | `docs/codex/00-index/previous_audit_json_provenance.md` | codex-prompts/00-index/P0045.md |
