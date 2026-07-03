
# AGENTS.md — COC AI TRPG Codex v2.21 自包含施工总约束

> 适用对象：Codex CLI / Codex Cloud / Codex IDE / 自动修复代理
> 当前基线日期：2026-07-01
> 本文件是仓库根持久化施工提示词。把本包交给 Codex 时，先读取本文件，再读取 `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`。

## 1. 当前权威顺序

1. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`。
2. 本文件与 `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`。
3. `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`。
4. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`。
5. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`。
6. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
7. `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`。
8. `stages/sXX-*/START_PROMPT.md`、`ACCEPTANCE_PROMPT.md`、`TEST_PLAN.md`、`TEST_DATA.md`。
9. `docs/codex/00-index/codex-persistent-context.md` 与 `docs/codex/00-index/codex-prompt-boundary.md`。
10. `batches/B###.md` 与 `codex-prompts/<category>/P####.md`。
11. `docs/codex/**` 其他分类文档与 `source-archive/**` provenance 文档。

任何历史 V3/V4/V5/V6 报告、旧修复记录、旧 manifest 或旧 validation 与上述文件冲突时，必须按上述顺序处理。旧版本 token 只可作为 provenance，不得成为当前产品范围、工程命名或验收标准。

## 2. 不可突破红线

- V1 首发目标是 **COC 7 完整可玩闭环**，不是轻量 demo。
- HUMAN_KP / AI_KP 是 Campaign 级互斥权威模式；Authority Contract 创建后不可修改，只能 fork。
- 业务层、KP 服务、规则引擎、前端均不得直接调用 OpenAI / Ollama / llama.cpp / 任意裸 LLM。
- 所有 AI 能力必须走 `Agent Gateway -> Agent Orchestrator/Runtime -> Model Provider Adapter`。
- Agent 不能直接写数据库、伪造骰子、绕过规则引擎、绕过状态服务、绕过事件日志、泄露私密内容或修改 Authority Contract。
- 正式状态写入必须走 `Command -> Workflow -> Decision -> Event Store -> Projection`。
- Event Store 是正史；Projection、Cache、RAG Index、Summary 均为可重建读模型。
- Visibility Label 与 Fact Provenance 必须贯穿 API、Event、Agent Context、Tool Result、RAG、Summary、Export、Replay、Log、Metric。
- 所有正式骰子由服务端生成；AI/KP/前端不得编造或无记录改骰。
- 本地模型是一等 Provider，但未通过 Level 4 认证的本地模型不能担任 AI Keeper Orchestrator。
- 不得从本地模型静默 fallback 到云端模型；跨隐私边界必须显式配置、提示并审计。
- 生产环境不得接受占位 API key 或暴露未鉴权本地模型服务。

## 3. Codex 输出边界

- `primary-implementation` prompt 才能创建或修改 concrete Rust `src/`、`tests/`、migration、API handler、NATS subject、workflow 代码。
- `supplemental-requirement` prompt 只能生成补充需求 Markdown，并归并到对应 primary prompt。
- `documentation-or-traceability` prompt 只维护 Markdown、索引、矩阵、报告、验证清单或批次计划。
- Rust module、文件名、migration 名、event schema、NATS subject、metric label、测试名不得来自源文档路径、旧中间文档名、历史版本 token、hash 片段或截断尾缀。
- 执行任何 batch 或 per-file prompt 前，必须先应用 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。

## 4. 阶段执行纪律

每个阶段必须执行：

```text
读取本 AGENTS.md
读取 CODEX_STANDALONE_BOOTSTRAP_PROMPT.md
读取 SOURCE_BUNDLE_INTEGRATION_GUIDE.md
读取 docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md
读取 docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md
读取 docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
读取阶段 START_PROMPT.md
读取阶段 TEST_PLAN.md 和 TEST_DATA.md
读取阶段关联 docs/codex 分类、batch、per-file prompts
输出阶段施工计划
按最小可验证切片实现
运行阶段测试
生成阶段证据
执行阶段 ACCEPTANCE_PROMPT.md
若失败，执行 REPAIR_PROMPT.md
```

## 5. 失败修复纪律

不得通过以下方式“修复”失败：删除测试、弱化 policy gate、关闭 visibility 检查、允许业务层直连 LLM、让 Agent 直接写正式状态、让 projection 成为正史、让本地模型静默 fallback 到云端、把旧报告重新声明为当前验收入口、把旧 V3/V4/V5/V6 token 转为当前 module/output/migration/event/NATS/metric/test 名称。


## v2.21 strict repair note

本包保留全部提供文件的可追溯性：原始 V6 路径若因旧版本、旧 hash 或历史命名被规范化重命名，则其原始路径副本进入 `source-archive/v6-paths/**`，只用于审计与覆盖证明。Codex 当前执行只允许读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`、`docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md` 之后再进入 batch 或 per-file prompt。

## v2.21 strict provenance boundary

`source-archive/v6-legacy/**` 是只读原始输入证明区，不是可执行 prompt 区。任何旧版本 module/output/path 只能在该目录中作为 provenance 出现，当前施工必须使用 normalized overlay。

## v2.21 Codex 操作指南要求

Codex 执行任何工程实现前，必须读取 `CODEX_MASTER_EXECUTION_GUIDE.md`、`CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md`、`CODEX_STRICT_OPERATION_CHECKLIST.md`、`codex-operator-guides/README.md`。这些指南不降低任何顶层设计、Authority Contract、Agent Gateway、Visibility、Fact Provenance、Event Log 或 V1 Acceptance 约束。
