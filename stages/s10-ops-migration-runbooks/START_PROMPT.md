# S10 START_PROMPT — Ops / Migration：备份、恢复、升级、回滚、Projection Rebuild

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


你是 Codex，正在启动 `S10 — Ops / Migration：备份、恢复、升级、回滚、Projection Rebuild`。

## 必须读取

1. `AGENTS.md`
2. `docs/codex/00-index/codex-persistent-context.md`
3. `docs/codex/00-index/codex-prompt-boundary.md`
4. `S10` 阶段 README
5. 相关分类：`docs/codex/11-ops-migration`
6. 相关 batch：BATCH-042-11-ops-migration, BATCH-043-11-ops-migration
7. 相关 per-file prompts：`codex-prompts/11-ops-migration/**`

## 阶段目标

实现运维 runbook、migration/upgrade/rollback、backup/restore、projection rebuild、incident response、release checklist 与操作验证脚本。

## 输出边界

- Primary implementation 才能创建 concrete Rust src/test 输出。
- Supplemental requirement 只能写补充需求 Markdown 并归并到 primary。
- Documentation/traceability 只维护 Markdown、索引、矩阵、报告或验证清单。
- 保留 Authority、Event Store、Visibility、Fact Provenance、Policy Gate、Agent Gateway 约束。

## 请先输出施工计划

请先输出：

1. 已读取的文件清单。
2. 关联 batch 与 Prompt ID 覆盖方式。
3. 将修改的 crate、migration、schema、API/Event/WS/NATS、测试与 docs。
4. primary / supplemental / documentation 的处理策略。
5. 预计运行的测试命令。
6. 风险与需要人工确认的事项。

随后按最小可验证切片施工。
