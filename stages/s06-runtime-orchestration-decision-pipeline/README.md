# S06 — Runtime Orchestration：Session、Workflow、Pending Decision、Decision Commit Pipeline

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现 Campaign Session Runtime、workflow engine、pending decision、capability/tool grant、saga/transaction、scheduler、realtime binding 与 Decision Commit Pipeline。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S06` |
| 相关分类 | `03-runtime-orchestration` |
| 相关 batch | BATCH-012-03-runtime-orchestration, BATCH-013-03-runtime-orchestration, BATCH-014-03-runtime-orchestration, BATCH-015-03-runtime-orchestration, BATCH-016-03-runtime-orchestration |
| Prompt 数 | 115 |
| Primary | 26 |
| Supplemental | 64 |
| Docs/Trace | 25 |
| 主要 crate | `trpg-runtime` |

## 3. 启动条件

S02/S03/S04/S05 通过；domain/event/policy/ruleset 均可被 runtime 调用。

## 4. 主要输出

- `crates/trpg-runtime/src/*.rs`
- `crates/trpg-runtime/tests/*.rs`
- `docs/runtime/*.md`

## 5. 测试重点

- Command -> Workflow -> Decision -> Event Store -> Projection 链路
- pending decision 人类确认/拒绝
- AI_KP 正式工具请求允许/拒绝
- HUMAN_KP AI 草稿降级
- scheduler timeout/retry
- saga rollback/compensation

## 6. 推荐命令

- `cargo test -p trpg-runtime --all-features`
- `cargo test --test runtime_pending_decision`
- `cargo test --test workflow_engine_contract`

## 7. 测试数据

- `test-data/agent_tool_call_cases.md`
- `test-data/event_store_stream_cases.md`
- `test-data/authority_contract_cases.md`

## 8. 阶段验收清单

- [ ] 任何正式状态变更必须经过 Decision Commit Pipeline
- [ ] HUMAN_KP 模式 AI 输出 requires_human_confirmation=true
- [ ] AI_KP 模式只有 AI Keeper Orchestrator 可请求正式裁定工具
- [ ] 所有写命令保留幂等、版本、actor、correlation/causation 信息

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
