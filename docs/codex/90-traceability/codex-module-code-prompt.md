> [v2.21 自包含来源清理标记]
> 原始路径：`docs/codex/90-traceability/codex-module-code-prompt.md`
> 筛选状态：`active-traceability`
> 清理日期：2026-07-01
> 使用规则：当前可引用：可由 Codex 读取并参与施工，但必须服从顶层设计与 v2.21 阶段门禁。
> 过时信息处理：正文中出现的 `V4`、`V5`、早期 audit/fix/report 标题、源文档 hash、旧中间路径与历史版本流水仅表示 provenance，不得作为当前产品范围、命名规则或验收标准。若与顶层设计、`AGENTS.md`、`CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`、`SOURCE_BUNDLE_INTEGRATION_GUIDE.md` 或 `V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 冲突，以后者为准。

> [v2.21 当前执行规范化覆盖]
> 执行任何 batch、category prompt 或 per-file prompt 前，必须先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md` 与 `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
> 正文中的 V3/V4/V5/V6、v3/v4/v5/v6、legacy、fix-history、旧 manifest、旧 report、旧 hash 与旧中间路径仅保留为 provenance。任何 Rust module、输出文件、migration、event schema、NATS subject、metric label、测试名或验收入口必须采用 v2.21 normalized current-safe 名称。

# 90-traceability — Codex 模块编码提示词

```text
你是 Codex，负责 `90-traceability` 模块施工。

模块范围：Traceability / source audit / requirement-to-test mapping
默认 crate：trpg-docs-governance
模块前缀：traceability

执行要求：
1. 先读仓库根 AGENTS.md、docs/codex/00-index/codex-persistent-context.md、本模块 AGENTS.md。
2. 再按 `codex-prompts/90-traceability/` 中的 per-file prompt 执行。
3. 追踪文档必须能证明每个源文件、需求、测试和 prompt 的闭环。
4. 所有公开 Rust 类型使用领域专名；禁止 ModuleService/ModuleCommand/ModuleError 等模板残留。
5. 所有正式写入都必须接入 CommandEnvelope、workflow/decision、SQLx Event Store append、outbox/projection。
6. 每个实现任务必须同步更新测试、schema、文档追踪和 observability。
7. Rust module 文件布局必须二选一：默认使用 `src/<module>.rs`；只有迁移为多文件子模块时才使用目录式 `mod.rs`，且不得两者并存。

输出：
- 完成的 Prompt ID。
- 变更文件清单。
- 测试命令与结果。
- 未完成风险。
```

## 第四次严格修复：输出路径一致性

- 对 `product-code`、`test-harness`、`ops-runbook` prompt，Rust 输出文件必须由 `建议 crate` 与 `建议 Rust module` 推导：`crates/<crate>/src/<module_tail>.rs` 与对应 contract test。
- 不得从源文档路径、旧 V5 中间文件名、截断路径或 hash 片段生成 `docs_implementation_*`、`source_breakdown_*`、`generated_from_source_*` 伪 Rust 文件。
- 同一 concrete Rust 输出路径只能有一个 `primary-implementation` prompt；其他同 module prompt 必须是 `supplemental-requirement`，不得重复创建同一 `.rs` 文件。
- `docs-governance` 与 `traceability-maintenance` prompt 默认只维护 Markdown、索引、矩阵、报告、校验清单和追踪材料。

## 第五次严格修复：Supplemental 语义与稳定 module 命名

- `primary-implementation` 是唯一可以声明 concrete Rust src/test 输出文件的角色。
- `supplemental-requirement` 只生成 `docs/codex/90-traceability/supplemental-requirements/<Prompt ID>.md`，不得创建、声明、修改或要求新增 Rust src/test 输出。
- Supplemental 需要影响代码时，只能写入 merge instruction，并归并到对应 Primary Prompt。
- Primary Rust module 与 src/test 文件名不得来自源文档路径、旧中间文档名、版本流水、截断尾缀或 hash 片段。
- 严格校验必须覆盖 supplemental 正文语义、source-path-like module 名称、短 hash 残片、module/output 一致性、batch 覆盖和 manifest 自洽。
