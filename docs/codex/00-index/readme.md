> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/00-index/readme.md`
> 筛选状态：`active-index`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# 00-index — Codex 模块施工目录

范围：项目索引与施工总控
默认 crate：`trpg-docs-governance`
默认 module prefix：`docs_governance`
Per-file prompts：48

## 模块关键要求

索引、边界与映射文档必须保持与实现、测试、prompt ID 同步。

## 入口文件

- `AGENTS.md`
- `codex-module-code-prompt.md`
- `codex-module-test-prompt.md`
- `codex-module-review-prompt.md`
- per-file prompts：`codex-prompts/00-index/`

## BATCH-001 current-safe coverage

- Batch: `BATCH-001-00-index`
- Prompt count: 25
- Primary prompts: 0
- Allowed output: Markdown documentation and traceability only
- Evidence path: `evidence/batches/BATCH-001/`

## BATCH-002 current-safe coverage

- Batch: `BATCH-002-00-index`
- Prompt count: 23
- Primary prompts: 0
- Allowed output: Markdown documentation, index governance, traceability, and
  evidence only
- Evidence path: `evidence/batches/BATCH-002/`
