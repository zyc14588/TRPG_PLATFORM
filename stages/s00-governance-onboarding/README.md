# S00 — 治理落位与 Codex 施工入口

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

把顶层设计、V6 strict governance 文档、AGENTS、persistent context、prompt boundary、batch plan、traceability 索引落入仓库，建立 Codex-only 施工路径。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S00` |
| 相关分类 | `00-index`, `90-traceability`, `99-appendix` |
| 相关 batch | BATCH-001-00-index, BATCH-002-00-index, BATCH-046-90-traceability, BATCH-047-90-traceability, BATCH-048-90-traceability, BATCH-049-90-traceability, BATCH-050-90-traceability, BATCH-051-99-appendix, BATCH-052-99-appendix |
| Prompt 数 | 191 |
| Primary | 0 |
| Supplemental | 0 |
| Docs/Trace | 191 |
| 主要 crate | `trpg-docs-governance` |

## 3. 启动条件

已有顶层设计 Markdown 与 V6 Codex strict governance zip；尚未创建产品代码时也可启动。

## 4. 主要输出

- `AGENTS.md`
- `docs/codex/**`
- `docs/architecture/top-level-design.md`
- `docs/engineering/construction-plan/**`
- `docs/adr/**`
- Optional local helpers under `scripts/validate_codex_prompts.*` when present;
  S00 strict acceptance is recorded through batch evidence under
  `evidence/batches/BATCH-002/`.

## 5. 测试重点

- Markdown 链接检查
- Prompt ID 唯一性检查
- batch prompt 覆盖检查
- primary/supplemental 边界检查
- manifest 校验

## 6. 推荐命令

- Active strict checks for S00 are the inline inventory, prompt-row,
  traceability, fixture, Markdown evidence-link, and docs-only boundary checks
  recorded in `evidence/batches/BATCH-002/test-output.txt`.
- `python scripts/validate_codex_prompt_inventory.py` is an optional local
  helper when present; its absence is not an S00 blocker.
- `python scripts/validate_markdown_links.py` is an optional local helper when
  present; its absence is not an S00 blocker.
- Cargo checks are not applicable to S00 while this checkout has no
  `Cargo.toml` and the active S00 batch contains only
  `documentation-or-traceability` prompts. Product-code stages must run Cargo
  once a workspace manifest exists.

S00 docs-only applicability: for `BATCH-002-00-index`, product-code
Unit/Integration/Contract/Golden test layers are represented by the active
traceability, fixture, evidence-link, sensitive-label, and docs-only boundary
checks above. Rust/Cargo-backed variants are not applicable until a
product-code batch owns a Cargo workspace.

## 7. 测试数据

- `test-data/prompt_inventory_fixture.md`
- `test-data/change_control_cases.md`

## 8. 阶段验收清单

- [ ] 仓库根 AGENTS.md 存在并声明不可突破原则
- [ ] docs/codex/00-index/codex-persistent-context.md 与 prompt-boundary 可被 Codex 定位
- [ ] 52 个 batch 的覆盖表保留或导入
- [ ] 文档治理不得创建业务 Rust src/test 输出

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
