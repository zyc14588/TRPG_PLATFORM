# S05 — COC7 Ruleset：角色、骰子、检定、SAN、战斗、追逐、场景结构

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现 ruleset-agnostic core 的 COC7 包：角色卡/车卡、服务端骰子、技能检定、奖励/惩罚骰、Luck/Pushed Roll、SAN/疯狂、基础战斗与追逐、Scenario YAML/JSON schema。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S05` |
| 相关分类 | `05-ruleset-coc7` |
| 相关 batch | BATCH-021-05-ruleset-coc7, BATCH-022-05-ruleset-coc7, BATCH-023-05-ruleset-coc7 |
| Prompt 数 | 65 |
| Primary | 16 |
| Supplemental | 33 |
| Docs/Trace | 16 |
| 主要 crate | `trpg-ruleset-coc7` |

## 3. 启动条件

S01/S02 shared/domain 类型稳定；S03 event persistence 可用于规则结果落库。

## 4. 主要输出

- `crates/trpg-ruleset-coc7/src/*.rs`
- `rulesets/coc7/*.json`
- `schemas/scenario.schema.json`
- `crates/trpg-ruleset-coc7/tests/*.rs`

## 5. 测试重点

- COC7 属性派生与车卡合法性
- 服务端骰子成功等级
- 奖励/惩罚骰
- SAN 损失与疯狂状态
- 战斗/追逐状态机
- Scenario Validator 核心线索可达性

## 6. 推荐命令

- `cargo test -p trpg-ruleset-coc7 --all-features`
- `cargo test -p trpg-ruleset-coc7 dice`
- `cargo test -p trpg-ruleset-coc7 sanity`

## 7. 测试数据

- `test-data/tutorial_scenario_yaml.md`
- `test-data/golden_scenario_yaml.md`
- `test-data/dice_san_combat_chase_cases.md`

## 8. 阶段验收清单

- [ ] 所有正式骰子由服务端生成
- [ ] 核心线索不因一次失败永久丢失
- [ ] V1 单房间只允许 ruleset_id=coc7
- [ ] COC 专有字段不得污染 ruleset-agnostic core

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
