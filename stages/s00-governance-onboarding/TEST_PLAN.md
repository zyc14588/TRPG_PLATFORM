# S00 TEST_PLAN — 治理落位与 Codex 施工入口

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 测试目标

- Markdown 链接检查
- Prompt ID 唯一性检查
- batch prompt 覆盖检查
- primary/supplemental 边界检查
- manifest 校验

## 推荐命令

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

## 必须补齐的测试类型

- Unit：领域纯逻辑、错误码、状态机。
- Integration：数据库、事务、policy、provider、缓存、消息。
- Contract：API/Event/WS/NATS/schema。
- Negative：权限拒绝、version 冲突、幂等冲突、visibility 泄露、prompt injection。
- Golden/Replay：可重放一致性、导出 snapshot、projection hash。

## 阶段测试数据

- `test-data/prompt_inventory_fixture.md`
- `test-data/change_control_cases.md`

## 失败处理

用同目录 `REPAIR_PROMPT.md`。不得删除测试、关闭 policy gate、绕过 Event Store 或弱化 visibility redaction。
