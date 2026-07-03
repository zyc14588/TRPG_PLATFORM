> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/00-index/codex-module-review-prompt.md`
> 筛选状态：`active-index`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# 00-index — Codex 模块评审 / 修复提示词

```text
审查 `00-index` 模块变更，重点寻找：
- 绕过 Command -> Workflow -> Decision -> Event Store 的写路径。
- HUMAN_KP / AI_KP 混用。
- Authority Contract 原地修改。
- Visibility / Fact Provenance 丢失或泄露。
- 缺少 OpenFGA/OPA/Tool Grant/Policy Gate。
- 缺少 idempotency、expected_version、correlation_id、causation_id。
- 模板类型名残留或 serde_json::Value 域模型滥用。
- 缺少测试、schema、migration、observability、audit log。

输出 P0/P1/P2 findings，并给出最小修复 patch。
```
