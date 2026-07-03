# Source Bundle Integration Guide — v2.21 自包含源材料整合指南

## 1. 目标

本指南用于把本 Markdown-only 施工包放入目标仓库，让 Codex 不依赖外部附件、旧 zip、聊天上下文或人工记忆即可分阶段施工。

## 2. 目标仓库落库映射

| 本包路径 | 目标仓库路径 | 处理方式 | 说明 |
|---|---|---|---|
| `AGENTS.md` | `AGENTS.md` | 覆盖或合并到根文件 | Codex 根持久化约束。 |
| `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md` | `docs/construction/CODEX_STANDALONE_BOOTSTRAP_PROMPT.md` | 复制 | Codex 首次施工入口。 |
| `SOURCE_BUNDLE_INTEGRATION_GUIDE.md` | `docs/construction/SOURCE_BUNDLE_INTEGRATION_GUIDE.md` | 复制 | 源材料落库说明。 |
| `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md` | `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md` | 复制 | 当前产品与架构最高约束之一。 |
| `docs/codex/**` | `docs/codex/**` | 复制 | 已清理筛选并执行 v2.21 命名规范化的 Codex 稳定施工材料。 |
| `stages/**` | `docs/construction/stages/**` | 复制 | 14 阶段施工控制文件。 |
| `prompts/persistent/**` | `docs/construction/prompts/persistent/**` | 复制 | 长期辅助提示词。 |
| `fixtures/**` | `tests/fixtures/**` 或 `crates/trpg-testing/fixtures/**` | 复制后由 Codex 转为实际 `.yaml/.json` | Markdown-only 包内以 `.md` 包装，落库时提取代码块。 |
| `ci-cd/workflows-extractable/*.md` | `.github/workflows/*.yml` | 提取 fenced `yaml` | 由 `CI_CD_EXTRACTION_PROMPT.md` 指导。 |
| `inventory/**` | `docs/construction/inventory/**` | 复制 | 输入筛选、Prompt 覆盖和审计依据。 |
| `manifests/**` | `docs/construction/manifests/**` | 复制 | v2.21 当前 manifest 与校验依据；`source-archive/superseded/**` 不得作为当前 manifest。 |

## 3. 当前权威与过时信息处理

`docs/codex/**` 中保留了原 V6 strict governance 包的大量 per-file prompts。部分文件正文会出现 `V3`、`V4`、`V5`、`V6`、旧修复报告、历史 manifest、旧 SHA 或旧中间路径。这些内容仅用于 provenance 与 prompt 追踪，不是当前目标版本，也不得覆盖顶层设计。

```text
当前实现目标 = 顶层设计 + v2.21 阶段施工方案 + v2.21 normalized maps + V6 strict governance 的 active prompt/batch 约束
历史版本词 = provenance，不是产品范围
历史 fix-history/report = quarantined，不是验收入口
```

## 4. 严格合并流程

1. 解压本包到目标仓库外的临时目录。
2. 用 Codex 读取 `AGENTS.md` 与 `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`。
3. 按本文件的映射复制文档和 fixture。复制后先读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`、`CURRENT_TOKEN_REWRITE_TABLE.md`。
4. 执行 `CI_CD_EXTRACTION_PROMPT.md`，把 workflow Markdown 提取为 `.github/workflows/*.yml`。
5. 执行 `stages/s00-governance-onboarding/START_PROMPT.md`。
6. 每阶段完成后运行对应 `ACCEPTANCE_PROMPT.md` 并把证据写入 `docs/reports/stages/SXX_ACCEPTANCE_EVIDENCE.md`。
7. S13 前不得跳过任何阶段的 P0/P1 证据。

## 5. 禁止事项

- 禁止只复制 `stages/**` 而不复制 `docs/codex/**`。
- 禁止把 `source-archive/quarantined/**` 作为当前施工入口。
- 禁止把旧 V3/V4/V5/V6 文件名、旧 hash 或旧路径转为 Rust module、migration、event schema、NATS subject、metric label 或测试名。
- 禁止跳过 `batches/B###.md` 和 `per-file-prompts/**` 的 primary/supplemental/documentation 角色边界。


## v2.21 strict repair note

本包保留全部提供文件的可追溯性：原始 V6 路径若因旧版本、旧 hash 或历史命名被规范化重命名，则其原始路径副本进入 `source-archive/v6-paths/**`，只用于审计与覆盖证明。Codex 当前执行只允许读取 `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`、`docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`、`docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md` 之后再进入 batch 或 per-file prompt。

## v2.21 严格修复补充：原始旧路径 provenance

`source-archive/v6-legacy/**` 只用于证明已经遍历并保留原始 V6 输入中被规范化改名的 prompt 文件。该目录不属于当前执行路径。Codex 不得从该目录提取当前 Rust module、migration、NATS subject、metric、测试名或 suggested output。当前执行命名只能来自：

```text
codex-active-normalized/**
docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md
docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md
docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
```
