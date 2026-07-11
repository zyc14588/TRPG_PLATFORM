# BATCH-052 工作计划

Batch: `BATCH-052-99-appendix — Strict Governance Final`
Stage: `S00 — 治理落位与 Codex 施工入口`
Prompt 数量: 8
Primary / Supplemental: 0 / 0
Documentation-or-traceability: 8
Current-safe 唯一目标: 8

## 范围

本批 8 项全部是 `docs-governance` / `documentation-or-traceability`。只允许
更新下表列出的 current-safe Markdown 目标与
`evidence/batches/BATCH-052/`；不得创建或修改 Rust `src/`、产品测试、
migration、API/Event/WS/NATS 契约、metric、workflow、provider adapter 或
正式状态写入路径。

历史版本、旧路径和源 SHA 只能作为 provenance。两份 current-safe 映射优先于
`batches/B052.md` 与 category manifest 的较低优先级建议：P0029 必须使用
`appendix_research::prototype_catalog_previous_provenance` /
`prototype_catalog_previous.md`，P0032 必须使用
`appendix_research::research_decision_matrix_previous_provenance` /
`research_decision_matrix_previous.md`。

## 测试责任代码

- `D1`：Prompt ID、prompt 路径、source path/SHA、crate/module/output 与两份
  current-safe map 逐项一致。
- `D2`：8 个目标存在，Markdown H1、表格、fence、相对链接有效；6 个共享目标
  保留 B051 owner 并唯一追加 B052 owner。
- `D3`：docs-only 边界成立；不激活历史 Rust/SQL/API/event/NATS/metric/test
  提案，不削弱 Authority、Agent Gateway、Event Store、Visibility、Fact
  Provenance 或 Policy Gate。
- `D4`：P0029 与 P0032 明确为 previous/provenance，既非当前 backlog、技术
  决策、测试目录，也非阶段/V1 验收入口。
- `S00`：运行治理边界 verifier，并执行适用的 workspace fmt/check/clippy/test、
  visibility leakage 与根 pnpm gate。

## Prompt 映射、允许改动与测试责任

| Prompt ID | Prompt 文件 | Current-safe module | Current-safe 目标 | 允许改动范围 | 测试责任 |
|---|---|---|---|---|---|
| `CODEX-1100-99-APPENDIX-a6df9c35b5` | `P0025.md` | `appendix_research::document_template` | `document_template.md` | 保留 B051 模板，仅追加 B052 metadata 与责任 | D1/D2/D3/S00 |
| `CODEX-1101-99-APPENDIX-d70d0fbeb4` | `P0026.md` | `appendix_research::followup_research_prompts` | `followup_research_prompts.md` | 保留模板且不启动调研，仅追加 B052 owner | D1/D2/D3/S00 |
| `CODEX-1102-99-APPENDIX-6512e2632c` | `P0027.md` | `appendix_research::glossary` | `glossary.md` | 保留当前术语，仅追加 B052 metadata 与责任 | D1/D2/D3/S00 |
| `CODEX-1103-99-APPENDIX-94a27a771a` | `P0028.md` | `appendix_research::implementation_doc_template` | `implementation_doc_template.md` | 保留 primary 授权边界，仅追加 B052 owner | D1/D2/D3/S00 |
| `CODEX-1104-99-APPENDIX-645d245a1f` | `P0029.md` | `appendix_research::prototype_catalog_previous_provenance` | `prototype_catalog_previous.md` | 新建只读历史原型目录 provenance；不复制或执行旧任务 | D1/D2/D3/D4/S00 |
| `CODEX-1105-99-APPENDIX-853faddcb9` | `P0030.md` | `appendix_research::open_source_reference_notes` | `open_source_reference_notes.md` | 保留 advisory-only 内容，仅追加 B052 owner | D1/D2/D3/S00 |
| `CODEX-1106-99-APPENDIX-ef131affa7` | `P0032.md` | `appendix_research::research_decision_matrix_previous_provenance` | `research_decision_matrix_previous.md` | 新建只读历史决策矩阵 provenance；不重申当前选型 | D1/D2/D3/D4/S00 |
| `CODEX-1107-99-APPENDIX-5ec98d85a2` | `P0033.md` | `appendix_research::unresolved_questions` | `unresolved_questions.md` | 保留 open ledger 语义，仅追加 B052 owner | D1/D2/D3/S00 |

所有目标均位于 `docs/codex/99-appendix/`。

## 实施策略

1. 复用 B051 已验收的紧凑治理页面，不恢复 per-file prompt 中的历史实现草图。
2. 对 6 个共享目标只追加 B052 metadata/责任并保留全部 B051 行。
3. 对 2 个缺失目标创建最小 provenance-only 页面，明确重新进入施工必须由当前
   primary prompt 和阶段验收授权。
4. 不修改 B052 输入、category manifest 或 `source-archive/**`；映射差异记录在
   traceability 与风险中。

## 检查顺序

1. B052 最小检查：8 行、0 primary、8/8 map agreement、8 个目标、metadata、
   shared owner、previous 边界与 docs-only 扫描。
2. Markdown H1、表格、fence、相对链接和 whitespace 检查。
3. S00 detailed fixture：`scripts/verify-governance-boundary.ps1`。
4. Workspace：Cargo fmt/metadata/check/clippy/test、targeted visibility leakage、
   根 pnpm test 与 `git diff --check`。
5. 按 `batch-prompts/accept/B052.md` 独立复跑 B052 最小检查和 S00 gate。

SQLx 与 Docker 不适用：本批不拥有数据库、migration、compose、容器或部署面。
`trpg-docs-governance` 不是实际 Cargo package，因此不运行虚构的 package test。

## 已知风险

- B052 与 category manifest 仍保留 P0029/P0032 的低优先级旧建议；本批按两份
  一致的 current-safe map 覆盖，不越权重写输入。
- 历史版本片段、源路径和 SHA 只会保留在 provenance metadata。
- 本批为最后一个编号 batch；完成后不得自动启动新阶段或发布流程。
