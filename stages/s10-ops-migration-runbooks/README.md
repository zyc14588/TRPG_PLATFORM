# S10 — Ops / Migration：备份、恢复、升级、回滚、Projection Rebuild

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现运维 runbook、migration/upgrade/rollback、backup/restore、projection rebuild、incident response、release checklist 与操作验证脚本。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S10` |
| 相关分类 | `11-ops-migration` |
| 相关 batch | BATCH-042-11-ops-migration, BATCH-043-11-ops-migration |
| Prompt 数 | 43 |
| Primary | 11 |
| Supplemental | 23 |
| Docs/Trace | 9 |
| 主要 crate | `trpg-ops` |

## 3. 启动条件

S03/S09 可提供数据库、event store、projection、部署环境。

## 4. 主要输出

- `crates/trpg-ops/src/*.rs`
- `ops/runbooks/*.md`
- `scripts/backup_restore/*`
- `scripts/projection_rebuild/*`

## 5. 测试重点

- backup/restore 演练
- migration rollback
- projection rebuild 一致性
- 故障恢复 runbook dry-run
- release checklist 自动校验

## 6. 推荐命令

- `cargo test -p trpg-ops --all-features`
- `./scripts/backup_restore/smoke.sh`
- `./scripts/projection_rebuild/verify.sh`

## 7. 测试数据

- `test-data/event_store_stream_cases.md`
- `test-data/export_expected_snapshots.md`

## 8. 阶段验收清单

- [ ] 恢复后 Event Store hash 与备份前一致
- [ ] Projection rebuild 不产生新正史事件
- [ ] 升级失败可回滚到可启动版本
- [ ] runbook 记录权限、审计、隐私注意事项

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
