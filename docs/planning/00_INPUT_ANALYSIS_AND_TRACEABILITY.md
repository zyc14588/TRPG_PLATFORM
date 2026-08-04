# 00 — 输入解析、筛选与追踪结论 v2.21

## 1. 文件解析范围

本次修复重新遍历并筛选以下输入：

| 输入 | 文件数 | 处理结果 |
|---|---:|---|
| 顶层设计 Markdown | 1 | 复制到 `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`，作为当前产品/架构基线。 |
| V6 Codex strict governance 包 | 1299 | 全部 Markdown 已遍历；active 文档进入 `docs/codex/**`；旧报告/manifest/fix-history 进入 `source-archive/**` provenance/quarantine；active prompt 文件名与可执行 module/output 已完成 v2.21 规范化。 |
| 上版施工方案包 | 132 | 作为基础修复，补齐自包含源材料、V1 证据矩阵、fixture、CI/CD 提取、引用校验与 v2.21 normalized prompt 执行映射。 |

## 2. 原 V6 包筛选统计

| 状态 | 文件数 | 用途 |
| --- | --- | --- |
| active-batch | 52 | 52 个执行 batch 的组成部分；用于阶段内按批施工。 |
| active-domain-category | 72 | 工程分类入口、模块级 code/test/review prompt；阶段施工必读。 |
| active-index | 11 | Codex 持久化上下文、prompt boundary、batch plan、execution map；阶段启动必读。 |
| active-prompt | 1109 | Codex 细粒度任务契约；保留完整内容，旧版本词仅作为源文档 provenance。 |
| active-reference-overlaid | 1 | 原 AGENTS 作为参考；v2 根 AGENTS.md 已重写并继承其治理约束。 |
| active-traceability | 11 | Prompt/batch/ownership/traceability 索引；用于覆盖校验。 |
| appendix-reference | 11 | 附录/POC/参考材料；只在相应阶段需要时读取，不得覆盖顶层设计。 |
| quarantined-provenance | 25 | 历史修复/审计归档；只能用于追溯，不得作为当前施工入口。 |
| screened-provenance | 7 | 原 V6 包根报告/manifest；保留用于输入追踪，当前施工入口以 v2 根文档为准。 |


## 3. Prompt 统计

| 维度 | 统计 |
|---|---:|
| per-file prompt 总数 | 1109 |
| execution batch 总数 | 52 |
| primary implementation | 257 |
| supplemental requirement | 451 |
| documentation-or-traceability | 401 |

## 4. 清理规则

1. 所有嵌入的原 V6 Markdown 均追加 v2.21 当前执行规范化覆盖，声明当前权威顺序与 normalized map 门禁。
2. `V3`、`V4`、`V5`、`V6`、旧源路径、旧中间文档名、历史 hash、旧修复报告标题仅保留为 provenance，不得转化为当前工程命名或验收标准。
3. `docs/codex/90-traceability/fix-history/**` 不进入 active `docs/codex/**`，只进入 `source-archive/quarantined/**`。
4. 原 V6 根 `README/VALIDATION/MANIFEST/REPORT` 与 v2 旧 manifest 不作为当前施工入口，只作为 provenance。
5. 已移除旧 placeholder hash token 的歧义用法；当前测试 fixture 使用确定性 hash `91a0e921167aaa57c134c2b1bc549d4ff0c75f4a3f7503dd23a396726d63b831`。
6. 所有 Markdown 已规范换行、去除行尾空格，并保持 UTF-8。

## 5. 自包含验证结论

- `batches/B###.md`：52。
- `codex-prompts/<category>/P####.md`：1109。
- `stages/sXX-*`：14 个阶段，且每阶段均有 README、START_PROMPT、ACCEPTANCE_PROMPT、TEST_PLAN、TEST_DATA、REPAIR_PROMPT 六件套。
- `fixtures/**`：44 个 Markdown，其中 43 个数据 fixture + 1 个 README。

详细筛选清单见 `inventory/ALL_PROVIDED_FILE_SCREENING.md`。


## 6. v2.21 规范化修复摘要

| 修复项 | 结果 |
|---|---:|
| active per-file prompt 文件名旧版本 token 规范化 | 345 |
| actionable Rust module 行规范化 | 66 |
| actionable output 行规范化 | 62 |
| 旧 v2 manifest 归档 | 4 |
| 最新 v2 严格复核失败报告纳入 `source-archive/reviews/**` | 1 |

当前 Codex 执行前必须读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`、`docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`。
