# S07 — Agent Runtime：Gateway、Tool Permission Gate、Provider、本地模型认证、Memory/RAG

> [v2.21 自包含与规范化前置]
> 本阶段不再依赖外部原始 zip。所有必须读取的 Codex 源材料已经嵌入本包根 `docs/codex/**`。执行前必须先读取 `AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md`、`docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`、`docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`、`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 与 `PER_STAGE_FIXTURE_EXPANSION_PLAN.md`。正文中若出现 V3/V4/V5/V6、fix-history、旧 hash、旧路径或历史交付报告词汇，一律按 provenance 处理；任何当前 module/output/migration/event/NATS/metric/test 命名必须按 v2.21 normalized maps 改写，不得覆盖当前 v2.21 门禁。


## 1. 阶段目标

实现 Agent Gateway/Orchestrator/Runtime、Agent/Tool Registry、Context Assembler、Model Provider Adapter、local/cloud provider、No Silent Cloud Fallback、Local Model Certification、Memory/RAG、Golden Scenario evaluation。

## 2. 输入与 batch 覆盖

| 项 | 值 |
|---|---|
| 阶段编号 | `S07` |
| 相关分类 | `04-ai-agent-system` |
| 相关 batch | BATCH-017-04-ai-agent-system, BATCH-018-04-ai-agent-system, BATCH-019-04-ai-agent-system, BATCH-020-04-ai-agent-system |
| Prompt 数 | 95 |
| Primary | 28 |
| Supplemental | 45 |
| Docs/Trace | 22 |
| 主要 crate | `trpg-agent-runtime` |

## 3. 启动条件

S03/S04/S06 可提供 event/projection/policy/runtime 工具边界。

## 4. 主要输出

- `crates/trpg-agent-runtime/src/*.rs`
- `agent-packs/coc7/*.md`
- `crates/trpg-agent-runtime/tests/*.rs`
- `schemas/agent/*.json`

## 5. 测试重点

- 业务层不得直接调用 LLM 静态/架构测试
- Tool Permission Gate default deny
- Visibility context assembly 泄露负例
- Provider health/list/chat/tool-call/stream/capability probe
- local model certification Level 0-4
- no silent cloud fallback

## 6. 推荐命令

- `cargo test -p trpg-agent-runtime --all-features`
- `cargo test --test agent_tool_permission_gate`
- `cargo test --test model_certification_tests`

## 7. 测试数据

- `test-data/agent_tool_call_cases.md`
- `test-data/provider_model_certification_cases.md`
- `test-data/golden_scenario_yaml.md`

## 8. 阶段验收清单

- [ ] 所有 AI 能力经 Agent Gateway -> Orchestrator/Runtime -> Provider Adapter
- [ ] 表达 Agent 不能新增事实或调用状态变更工具
- [ ] 未认证本地模型不能担任 AI Keeper Orchestrator
- [ ] 跨本地/云端隐私边界的 fallback 必须显式配置并审计

## 9. 使用方式

先把 `START_PROMPT.md` 交给 Codex；实现后把变更 diff、测试日志和 `ACCEPTANCE_PROMPT.md` 交给 Codex 验收；失败时使用 `REPAIR_PROMPT.md`。
