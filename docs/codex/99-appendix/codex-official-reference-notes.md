> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/99-appendix/codex-official-reference-notes.md`
> 筛选状态：`appendix-reference`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# Codex 官方参考摘要

> 本页用于解释为什么本包采用 `AGENTS.md`、小批次 prompt、项目级配置模板和执行门禁。链接为官方 OpenAI / OpenAI Developers 页面。

## 摘要

- Codex CLI 是可在终端本地运行的编码代理，能在选定目录中读取、修改并运行代码。
- Codex 会在工作前读取 `AGENTS.md` 文件，因此本包生成仓库级 `AGENTS.md` 和模块级 `AGENTS.md` 作为持久化约束载体。
- Codex 支持项目级 `.codex/config.toml` 覆盖，但项目配置只在可信项目中加载；因此本包只提供 Markdown 模板，不在文档包中写入非 Markdown 配置文件。
- Codex 的 agent loop 会随工具调用增长上下文；本包将 1109 个输入文件拆为模块提示词与约 25 文件一个的 batch，避免单次上下文过大。

## 参考链接

- https://developers.openai.com/codex/cli
- https://developers.openai.com/codex/guides/agents-md
- https://developers.openai.com/codex/config-advanced
- https://openai.com/index/unrolling-the-codex-agent-loop/
