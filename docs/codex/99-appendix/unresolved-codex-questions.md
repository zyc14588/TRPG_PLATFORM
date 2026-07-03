> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/99-appendix/unresolved-codex-questions.md`
> 筛选状态：`appendix-reference`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# Unresolved Codex Questions

以下问题需要在真实代码仓库中由 Codex 执行时确认：

1. Cargo workspace 是否已经存在，以及 crate 名称是否与文档推荐完全一致。
2. SQLx migration 目录的实际路径和数据库连接配置。
3. CI 是否包含 nextest、cargo-deny、cargo-udeps、sqlx offline cache。
4. OpenAPI 生成流程是否使用 utoipa build script、xtask 还是独立命令。
5. NATS JetStream、OpenFGA、OPA、Ollama/pgvector 是否在本地 dev compose 中可用。
6. 部分 V5 per-file 文档来自旧 generated/strict/generated lineage，实际开发时是否需要合并同类 prompt。

处理原则：遇到不确定项时，Codex 应先检查仓库事实，再做最小实现；不要凭空创建与现有工程冲突的结构。
