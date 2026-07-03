# S02 — Domain Core：Authority、Campaign、Decision 与事件模型

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现 Campaign、AuthorityContract、Fork、Command/CQRS、DecisionRecord、GameEvent、CharacterSheetVersion、MemoryFact、Visibility 与 Fact Provenance 的领域模型和守卫。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S02` |
| 相关分类 | `02-domain-core` |
| 相关 batch | BATCH-007-02-domain-core, BATCH-008-02-domain-core, BATCH-009-02-domain-core, BATCH-010-02-domain-core, BATCH-011-02-domain-core |
| Prompt 数 | 106 |
| Primary | 34 |
| Supplemental | 50 |
| Docs/Trace | 22 |
| 主要 crate | `trpg-domain-core` |

## 3. 启动条件

S01 shared kernel 可编译；Authority/Visibility 基础类型已稳定。

## 4. 主要输出

- `crates/trpg-domain-core/src/*.rs`
- `crates/trpg-domain-core/tests/*_contract_tests.rs`
- `docs/domain/*.md`

## 5. 测试重点

- Authority Contract 不可变测试
- HUMAN_KP/AI_KP 互斥测试
- fork 不修改原 Campaign 事件测试
- DecisionRecord 与 GameEvent provenance 测试
- Visibility lattice 负例

## 6. 推荐命令

- `cargo test -p trpg-domain-core --all-features`
- `cargo test -p trpg-domain-core authority --all-features`
- `cargo test -p trpg-domain-core visibility --all-features`

## 7. 测试数据

- `test-data/authority_contract_cases.md`
- `test-data/visibility_leakage_cases.md`
- `test-data/fork_lineage_cases.md`

## 8. 阶段验收清单

- [ ] Campaign 生命周期内 authority_mode 和 authority_owner 不可更改
- [ ] 正式状态只能由 Decision/Event 路径生成
- [ ] Agent 草稿、玩家推理、NPC 台词不得写入 confirmed fact
- [ ] Fork 生成新 AuthorityContract 且原事件日志不变

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
