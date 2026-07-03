# S11 — Testing Quality：Tutorial/Golden Scenario、Contract、Leakage、Model Certification CI

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

建立 V1 质量门禁：Tutorial Scenario、Golden Scenario、contract tests、leakage tests、prompt injection tests、mode-difference tests、model certification tests、benchmark 与 trace map。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S11` |
| 相关分类 | `10-testing-quality` |
| 相关 batch | BATCH-038-10-testing-quality, BATCH-039-10-testing-quality, BATCH-040-10-testing-quality, BATCH-041-10-testing-quality |
| Prompt 数 | 78 |
| Primary | 24 |
| Supplemental | 37 |
| Docs/Trace | 17 |
| 主要 crate | `trpg-testing` |

## 3. 启动条件

S02-S10 主链路已经可编译运行；每阶段已有单测/集成测试。

## 4. 主要输出

- `crates/trpg-testing/src/*.rs`
- `tests/golden/*.rs`
- `fixtures/tutorial/*`
- `fixtures/golden/*`
- `docs/testing/*.md`

## 5. 测试重点

- Golden Scenario fixed input/replay
- Tutorial e2e
- HUMAN_KP/AI_KP 模式差异
- visibility/private export leakage
- prompt injection
- model certification Level 4
- benchmark

## 6. 推荐命令

- `cargo test -p trpg-testing --all-features`
- `cargo test --test golden_scenarios_ci`
- `cargo test --test visibility_leakage`
- `cargo test --test model_certification_tests`

## 7. 测试数据

- `test-data/tutorial_scenario_yaml.md`
- `test-data/golden_scenario_yaml.md`
- `test-data/export_expected_snapshots.md`
- `test-data/provider_model_certification_cases.md`

## 8. 阶段验收清单

- [ ] Golden Scenario Tests 全部通过
- [ ] 失败不能通过删除测试、弱化 policy gate 或关闭 visibility 检查解决
- [ ] 所有 V1 acceptance 项至少有一个自动测试或明确人工验收证据
- [ ] CI 生成 coverage、test artifact、decision trace map

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
