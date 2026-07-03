# S13 — V1 Release Hardening 与总验收

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

执行端到端 V1 acceptance：Docker Compose、一键初始化、模型配置、本地模型认证、AI/HUMAN KP Campaign、车卡、跑完整 Tutorial、跑 Golden、多人分组、导出、fork、审计、隐私、备份恢复。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S13` |
| 相关分类 | `ALL` |
| 相关 batch | ALL |
| Prompt 数 | 1109 |
| Primary | 257 |
| Supplemental | 451 |
| Docs/Trace | 401 |
| 主要 crate | `workspace` |

## 3. 启动条件

S00-S12 全部通过；CI 绿；未决 P0/P1 为零。

## 4. 主要输出

- `RELEASE_NOTES.md`
- `V1_ACCEPTANCE_REPORT.md`
- `docs/release/v1-acceptance-evidence.md`
- `artifacts/test-reports/*`

## 5. 测试重点

- workspace 全量 fmt/clippy/test
- compose smoke
- e2e tutorial
- golden scenario
- backup restore
- privacy/export audit
- provider boundary
- security negative tests

## 6. 推荐命令

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `docker compose up -d --build`
- `cargo test --test golden_scenarios_ci`

## 7. 测试数据

- `test-data/tutorial_scenario_yaml.md`
- `test-data/golden_scenario_yaml.md`
- `test-data/export_expected_snapshots.md`

## 8. 阶段验收清单

- [ ] 17 条 V1 完成标准逐项有证据
- [ ] P0/P1 defects=0
- [ ] 无静默云端 fallback
- [ ] AI_KP 正式裁定均有 tool/event/audit 记录
- [ ] 玩家版导出不含 keeper_only/private/ai_internal

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
