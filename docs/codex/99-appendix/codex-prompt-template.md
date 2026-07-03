> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/99-appendix/codex-prompt-template.md`
> 筛选状态：`appendix-reference`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# Codex Prompt Template

```text
你是 Codex，正在 COC AI TRPG Rust monorepo 执行施工。

必须先读取：
- AGENTS.md
- docs/codex/00-index/codex-persistent-context.md
- docs/codex/<category>/AGENTS.md
- codex-prompts/<category>/<prompt>.md

不可突破原则：
- Authority Contract 不可变；变更只能 Fork 或新建 authority_contract_version。
- HUMAN_KP / AI_KP 在 Campaign 级互斥，任何 handler 都不得在同一 campaign 同时接受两种 KP authority。
- AI 不直接写正式状态；AI 只能提出 Proposal / ToolCall / DraftDecision。
- 正式状态必须走 Command -> Workflow -> Decision -> Event Store -> Projection。
- Event Store 是正史，Projection / Cache / RAG Index 均为可重建读模型。
- Visibility Label 与 Fact Provenance 必须跨 API、Event、Agent、RAG、Export、Log、Metric 传播。
- Tool Grant、Policy Gate、OpenFGA、OPA、Audit Log 不得被 Agent、插件或外部 Provider 绕过。
- 所有写命令必须具备 idempotency_key、expected_version、actor、correlation_id、causation_id。

任务：
- Prompt ID: <PROMPT_ID>
- 目标 crate: <CRATE>
- 目标 module: <MODULE>
- 输出: code + tests + migrations/schema/docs + trace update

完成后报告：
- 变更文件
- 测试命令与结果
- schema/migration/event/API 变化
- 风险/TODO
```
