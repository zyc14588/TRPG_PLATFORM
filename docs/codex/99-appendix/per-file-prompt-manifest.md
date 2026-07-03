> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/99-appendix/per-file-prompt-manifest.md`
> 筛选状态：`appendix-reference`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# 99-appendix — Per-file Prompt Manifest（Strict Governance Final）

> Prompt 数量：33。第五次严格修复后，supplemental prompt 只补需求，primary prompt 才能拥有 Rust 输出文件。

| # | Prompt ID | 任务类型 | 输出角色 | Primary Prompt | module | 建议输出文件 | Prompt 文件 |
|---:|---|---|---|---|---|---|---|
| 1 | CODEX-1072-99-APPENDIX-b9f2731490 | docs-governance | documentation-or-traceability |  | appendix_research::document_template | `docs/codex/99-appendix/document_template.md` | codex-prompts/99-appendix/P0001.md |
| 2 | CODEX-1073-99-APPENDIX-35667c12b9 | docs-governance | documentation-or-traceability |  | appendix_research::followup_prompts | `docs/codex/99-appendix/followup_prompts.md` | codex-prompts/99-appendix/P0002.md |
| 3 | CODEX-1074-99-APPENDIX-6adee21312 | docs-governance | documentation-or-traceability |  | appendix_research::followup_research_prompts | `docs/codex/99-appendix/followup_research_prompts.md` | codex-prompts/99-appendix/P0003.md |
| 4 | CODEX-1075-99-APPENDIX-a3a2db4fc3 | docs-governance | documentation-or-traceability |  | appendix_research::docs_implementation_99_appendix_document_template_strict | `docs/codex/99-appendix/docs_implementation_99_appendix_document_template_strict.md` | codex-prompts/99-appendix/P0007.md |
| 5 | CODEX-1076-99-APPENDIX-1cf7a45b32 | docs-governance | documentation-or-traceability |  | appendix_research::chat_gpt | `docs/codex/99-appendix/chat_gpt.md` | codex-prompts/99-appendix/P0004.md |
| 6 | CODEX-1077-99-APPENDIX-dde42583c6 | docs-governance | documentation-or-traceability |  | appendix_research::docs_implementation_99_appendix_glossary_strict | `docs/codex/99-appendix/docs_implementation_99_appendix_glossary_strict.md` | codex-prompts/99-appendix/P0008.md |
| 7 | CODEX-1078-99-APPENDIX-f1cfd42c3c | docs-governance | documentation-or-traceability |  | appendix_research::docs_implementation_99_appendix_implementation_doc_template_strict | `docs/codex/99-appendix/docs_implementation_99_appendix_implementation_doc_template_strict.md` | codex-prompts/99-appendix/P0009.md |
| 8 | CODEX-1079-99-APPENDIX-8cfa2d0624 | docs-governance | documentation-or-traceability |  | appendix_research::open_source_reference_notes | `docs/codex/99-appendix/open_source_reference_notes.md` | codex-prompts/99-appendix/P0006.md |
| 9 | CODEX-1080-99-APPENDIX-65f846f412 | docs-governance | documentation-or-traceability |  | appendix_research::unresolved_questions | `docs/codex/99-appendix/unresolved_questions.md` | codex-prompts/99-appendix/P0005.md |
| 10 | CODEX-1081-99-APPENDIX-a93ccea365 | docs-governance | documentation-or-traceability |  | appendix_research::chat_gpt | `docs/codex/99-appendix/chat_gpt.md` | codex-prompts/99-appendix/P0010.md |
| 11 | CODEX-1082-99-APPENDIX-fa5d472e21 | docs-governance | documentation-or-traceability |  | appendix_research::glossary | `docs/codex/99-appendix/glossary.md` | codex-prompts/99-appendix/P0011.md |
| 12 | CODEX-1083-99-APPENDIX-cd4ea494b1 | docs-governance | documentation-or-traceability |  | appendix_research::implementation_doc_template | `docs/codex/99-appendix/implementation_doc_template.md` | codex-prompts/99-appendix/P0012.md |
| 13 | CODEX-1084-99-APPENDIX-74cacc0ed6 | docs-governance | documentation-or-traceability |  | appendix_research::prototype_catalog_strict | `docs/codex/99-appendix/prototype_catalog_strict.md` | codex-prompts/99-appendix/P0013.md |
| 14 | CODEX-1085-99-APPENDIX-a0cf91a645 | docs-governance | documentation-or-traceability |  | appendix_research::open_questions | `docs/codex/99-appendix/open_questions.md` | codex-prompts/99-appendix/P0014.md |
| 15 | CODEX-1086-99-APPENDIX-678a4fceb9 | docs-governance | documentation-or-traceability |  | appendix_research::open_source_reference_notes | `docs/codex/99-appendix/open_source_reference_notes.md` | codex-prompts/99-appendix/P0015.md |
| 16 | CODEX-1087-99-APPENDIX-504bf177bd | docs-governance | documentation-or-traceability |  | appendix_research::research_notes_2026_06_30 | `docs/codex/99-appendix/research_notes_2026_06_30.md` | codex-prompts/99-appendix/P0016.md |
| 17 | CODEX-1088-99-APPENDIX-7c8ccdfd7e | docs-governance | documentation-or-traceability |  | appendix_research::unresolved_questions | `docs/codex/99-appendix/unresolved_questions.md` | codex-prompts/99-appendix/P0017.md |
| 18 | CODEX-1089-99-APPENDIX-668b316c7b | docs-governance | documentation-or-traceability |  | appendix_research::document_template | `docs/codex/99-appendix/document_template.md` | codex-prompts/99-appendix/P0018.md |
| 19 | CODEX-1090-99-APPENDIX-110bb4e11e | docs-governance | documentation-or-traceability |  | appendix_research::followup_research_prompts | `docs/codex/99-appendix/followup_research_prompts.md` | codex-prompts/99-appendix/P0019.md |
| 20 | CODEX-1091-99-APPENDIX-6fac5c6fee | docs-governance | documentation-or-traceability |  | appendix_research::glossary | `docs/codex/99-appendix/glossary.md` | codex-prompts/99-appendix/P0020.md |
| 21 | CODEX-1092-99-APPENDIX-63da50377b | docs-governance | documentation-or-traceability |  | appendix_research::implementation_doc_template | `docs/codex/99-appendix/implementation_doc_template.md` | codex-prompts/99-appendix/P0021.md |
| 22 | CODEX-1093-99-APPENDIX-e48b12230c | docs-governance | documentation-or-traceability |  | appendix_research::open_source_reference_notes | `docs/codex/99-appendix/open_source_reference_notes.md` | codex-prompts/99-appendix/P0022.md |
| 23 | CODEX-1094-99-APPENDIX-368d7c729a | docs-governance | documentation-or-traceability |  | appendix_research::unresolved_questions | `docs/codex/99-appendix/unresolved_questions.md` | codex-prompts/99-appendix/P0023.md |
| 24 | CODEX-1095-99-APPENDIX-499db16dee | docs-governance | documentation-or-traceability |  | appendix_research::chatgpt_followup_research_prompts | `docs/codex/99-appendix/chatgpt_followup_research_prompts.md` | codex-prompts/99-appendix/P0024.md |
| 25 | CODEX-1099-99-APPENDIX-e2d7df571d | docs-governance | documentation-or-traceability |  | appendix_research::m_99_appendix | `docs/codex/99-appendix/m_99_appendix.md` | codex-prompts/99-appendix/P0031.md |
| 26 | CODEX-1100-99-APPENDIX-a6df9c35b5 | docs-governance | documentation-or-traceability |  | appendix_research::document_template | `docs/codex/99-appendix/document_template.md` | codex-prompts/99-appendix/P0025.md |
| 27 | CODEX-1101-99-APPENDIX-d70d0fbeb4 | docs-governance | documentation-or-traceability |  | appendix_research::followup_research_prompts | `docs/codex/99-appendix/followup_research_prompts.md` | codex-prompts/99-appendix/P0026.md |
| 28 | CODEX-1102-99-APPENDIX-6512e2632c | docs-governance | documentation-or-traceability |  | appendix_research::glossary | `docs/codex/99-appendix/glossary.md` | codex-prompts/99-appendix/P0027.md |
| 29 | CODEX-1103-99-APPENDIX-94a27a771a | docs-governance | documentation-or-traceability |  | appendix_research::implementation_doc_template | `docs/codex/99-appendix/implementation_doc_template.md` | codex-prompts/99-appendix/P0028.md |
| 30 | CODEX-1104-99-APPENDIX-645d245a1f | docs-governance | documentation-or-traceability |  | appendix_research::prototype_catalog_provenance | `docs/codex/99-appendix/prototype_catalog_provenance.md` | codex-prompts/99-appendix/P0029.md |
| 31 | CODEX-1105-99-APPENDIX-853faddcb9 | docs-governance | documentation-or-traceability |  | appendix_research::open_source_reference_notes | `docs/codex/99-appendix/open_source_reference_notes.md` | codex-prompts/99-appendix/P0030.md |
| 32 | CODEX-1106-99-APPENDIX-ef131affa7 | docs-governance | documentation-or-traceability |  | appendix_research::research_decision_matrix | `docs/codex/99-appendix/research_decision_matrix.md` | codex-prompts/99-appendix/P0032.md |
| 33 | CODEX-1107-99-APPENDIX-5ec98d85a2 | docs-governance | documentation-or-traceability |  | appendix_research::unresolved_questions | `docs/codex/99-appendix/unresolved_questions.md` | codex-prompts/99-appendix/P0033.md |
