# BATCH-051 工作计划

Batch: `BATCH-051-99-appendix — Strict Governance Final`  
Stage: `S00 — 治理落位与 Codex 施工入口`  
Prompt 数量: 25  
Primary / Supplemental: 0 / 0  
Documentation-or-traceability: 25  
Current-safe 唯一目标: 16

## 范围

本批所有条目均为 `docs-governance` / `documentation-or-traceability`。只允许
创建下表 current-safe Markdown 目标与 `evidence/batches/BATCH-051/`，不得创建
或修改 Rust `src/`、产品测试、migration、API/Event/WS/NATS 契约、metric、
workflow、provider adapter 或正式状态写入路径。

历史版本、旧路径与源 SHA 只保留在 provenance 字段。目标 module/output 一律
采用两份 current-safe 映射；尤其覆盖 `batches/B051.md` 中 P0002、P0007、
P0008、P0013、P0014 的较低优先级建议，并以 normalized map 修正 P0009 的
module。

## 测试责任代码

- `D1`：Prompt ID、prompt 路径、source path/SHA、crate/module/output 与两份
  current-safe map 逐项一致。
- `D2`：目标存在，Markdown H1、表格与 fence 结构有效；共享目标完整合并所有
  本批 row，且 Prompt ID 不重复。
- `D3`：docs-only 边界成立；不激活历史 Rust/SQL/API/event/NATS/metric/test
  提案，不削弱 Authority、Agent Gateway、Event Store、Visibility、Fact
  Provenance。
- `D4`：previous/provenance 目标明确声明非当前权威、非当前验收入口。
- `S00`：运行 `scripts/verify-governance-boundary.ps1` 与适用 workspace gate。

## Prompt 映射、允许改动与测试责任

| Prompt ID | Prompt 文件 | Current-safe module | Current-safe 目标 | 允许改动范围 | 测试责任 |
|---|---|---|---|---|---|
| `CODEX-1072-99-APPENDIX-b9f2731490` | `P0001.md` | `appendix_research::document_template` | `document_template.md` | 新建模板治理 trace；与 P0018 合并 | D1/D2/D3/S00 |
| `CODEX-1073-99-APPENDIX-35667c12b9` | `P0002.md` | `appendix_research::followup_prompts_previous_provenance` | `followup_prompts_previous.md` | 新建只读 provenance trace | D1/D2/D3/D4/S00 |
| `CODEX-1074-99-APPENDIX-6adee21312` | `P0003.md` | `appendix_research::followup_research_prompts` | `followup_research_prompts.md` | 新建调研提示词治理 trace；与 P0019 合并 | D1/D2/D3/S00 |
| `CODEX-1075-99-APPENDIX-a3a2db4fc3` | `P0007.md` | `appendix_research::docs_implementation_99_appendix_document_template_previous_provenance` | `docs_implementation_99_appendix_document_template_strict_previous.md` | 新建只读 provenance trace | D1/D2/D3/D4/S00 |
| `CODEX-1076-99-APPENDIX-1cf7a45b32` | `P0004.md` | `appendix_research::chat_gpt` | `chat_gpt.md` | 新建文档治理 trace；与 P0010 合并 | D1/D2/D3/S00 |
| `CODEX-1077-99-APPENDIX-dde42583c6` | `P0008.md` | `appendix_research::docs_implementation_99_appendix_glossary_previous_provenance` | `docs_implementation_99_appendix_glossary_strict_previous.md` | 新建只读 provenance trace | D1/D2/D3/D4/S00 |
| `CODEX-1078-99-APPENDIX-f1cfd42c3c` | `P0009.md` | `appendix_research::docs_implementation_99_appendix_implementation_doc_template_strict` | `docs_implementation_99_appendix_implementation_doc_template_strict.md` | 新建模板治理 trace；不得实现历史代码摘录 | D1/D2/D3/S00 |
| `CODEX-1079-99-APPENDIX-8cfa2d0624` | `P0006.md` | `appendix_research::open_source_reference_notes` | `open_source_reference_notes.md` | 新建参考资料 trace；与 P0015/P0022 合并 | D1/D2/D3/S00 |
| `CODEX-1080-99-APPENDIX-65f846f412` | `P0005.md` | `appendix_research::unresolved_questions` | `unresolved_questions.md` | 新建未决问题 trace；与 P0017/P0023 合并 | D1/D2/D3/S00 |
| `CODEX-1081-99-APPENDIX-a93ccea365` | `P0010.md` | `appendix_research::chat_gpt` | `chat_gpt.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1082-99-APPENDIX-fa5d472e21` | `P0011.md` | `appendix_research::glossary` | `glossary.md` | 新建术语治理 trace；与 P0020 合并 | D1/D2/D3/S00 |
| `CODEX-1083-99-APPENDIX-cd4ea494b1` | `P0012.md` | `appendix_research::implementation_doc_template` | `implementation_doc_template.md` | 新建模板治理 trace；与 P0021 合并 | D1/D2/D3/S00 |
| `CODEX-1084-99-APPENDIX-74cacc0ed6` | `P0013.md` | `appendix_research::prototype_catalog_previousstrict` | `prototype_catalog_previous-provenance.md` | 新建只读 prototype provenance trace | D1/D2/D3/D4/S00 |
| `CODEX-1085-99-APPENDIX-a0cf91a645` | `P0014.md` | `appendix_research::open_questions_previous_provenance` | `open_questions_previous.md` | 新建只读问题 provenance trace | D1/D2/D3/D4/S00 |
| `CODEX-1086-99-APPENDIX-678a4fceb9` | `P0015.md` | `appendix_research::open_source_reference_notes` | `open_source_reference_notes.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1087-99-APPENDIX-504bf177bd` | `P0016.md` | `appendix_research::research_notes_2026_06_30` | `research_notes_2026_06_30.md` | 新建带日期的 provenance/research trace | D1/D2/D3/S00 |
| `CODEX-1088-99-APPENDIX-7c8ccdfd7e` | `P0017.md` | `appendix_research::unresolved_questions` | `unresolved_questions.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1089-99-APPENDIX-668b316c7b` | `P0018.md` | `appendix_research::document_template` | `document_template.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1090-99-APPENDIX-110bb4e11e` | `P0019.md` | `appendix_research::followup_research_prompts` | `followup_research_prompts.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1091-99-APPENDIX-6fac5c6fee` | `P0020.md` | `appendix_research::glossary` | `glossary.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1092-99-APPENDIX-63da50377b` | `P0021.md` | `appendix_research::implementation_doc_template` | `implementation_doc_template.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1093-99-APPENDIX-e48b12230c` | `P0022.md` | `appendix_research::open_source_reference_notes` | `open_source_reference_notes.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1094-99-APPENDIX-368d7c729a` | `P0023.md` | `appendix_research::unresolved_questions` | `unresolved_questions.md` | 合并到共享目标，不建别名 | D1/D2/D3/S00 |
| `CODEX-1095-99-APPENDIX-499db16dee` | `P0024.md` | `appendix_research::chatgpt_followup_research_prompts` | `chatgpt_followup_research_prompts.md` | 新建提示词 provenance/trace；不执行调研 | D1/D2/D3/S00 |
| `CODEX-1099-99-APPENDIX-e2d7df571d` | `P0031.md` | `appendix_research::m_99_appendix` | `m_99_appendix.md` | 新建 appendix 目录治理 trace；不执行 B052 | D1/D2/D3/S00 |

所有目标均位于 `docs/codex/99-appendix/`。

## 实施策略

- 复用已验收 B050 的紧凑 trace-page 形式，不恢复 prompt 中的历史实现草图。
- 每个共享目标只创建一个文件，并在同一表中闭合全部 B051 Prompt ID。
- previous/provenance 目标明确隔离为只读来源证明。
- 只增加 16 个映射目标和批次证据；不更新 B052 行或其专属目标。

## 检查顺序

1. B051 最小检查：25 行、25/25 map agreement、16 个目标、metadata、共享目标、
   previous 边界、Markdown 结构和 docs-only 扫描。
2. S00 详细 fixture：`scripts/verify-governance-boundary.ps1`。
3. Workspace gate：Cargo fmt/check/clippy/test、visibility leakage、可用的根 pnpm
   测试与 `git diff --check`。
4. SQLx/Docker：本批不改数据库或部署面，记录 N/A。
5. 按 `batch-prompts/accept/B051.md` 独立复跑最小检查和 S00 验收。

## 已知风险

- B051 表、category manifest 与部分 prompt 正文仍保留被 current maps 覆盖的旧建议
  module/output；本批不越权重写这些输入。
- B052 与部分 current-safe 目标共享所有权；本批只写 B051 row，B052 必须在独立
  会话中追加自己的 row，不得把本批结论当作 B052 完成证据。
