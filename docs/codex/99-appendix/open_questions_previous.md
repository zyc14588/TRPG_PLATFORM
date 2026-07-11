# BATCH-051 历史未决问题 Provenance

> 本页不是当前 backlog、实现入口或验收入口。历史问题只有经过当前权威材料重新确认后，才能进入新的施工任务。

## Current-safe metadata

| Prompt ID | Prompt path | Source path | Source SHA-256 | Crate | Current-safe module | Current-safe output |
|---|---|---|---|---|---|---|
| CODEX-1085-99-APPENDIX-a0cf91a645 | codex-prompts/99-appendix/P0014.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-open-questions-v4-711afa7cdb.v5-code-ready.md | 6674bd330d100fa8b8caf7bdbb1b61a932d5325ec02a49b697a28ca9c5d2f4b4 | trpg-docs-governance | appendix_research::open_questions_previous_provenance | docs/codex/99-appendix/open_questions_previous.md |

源路径、版本片段和 SHA 仅用于 provenance。

## Allowed boundary

- 只记录历史问题及其 current-safe 处置。
- 不把旧问题自动升级为当前 V1 scope、实现任务或验收标准。
- 不在缺少当前 owner、权威输入、决策和证据时推测问题已解决。

## 历史问题处置

| 历史问题 | Current-safe disposition | 当前权威依据 |
|---|---|---|
| 规则版权边界如何落地 | 当前产品只提供规则包结构、角色卡模板、骰子逻辑和用户自定义导入机制；不默认内置未授权商业规则书或模组全文。本页不创建实现任务。 | CURRENT_TOP_LEVEL_DESIGN.md 的数据、隐私、版权与删除边界 |

未在本页逐项列出的历史问题仍保持 provenance 状态，不得从旧标题、旧报告或源路径推断为当前未决项。

新问题若需进入当前施工，至少应记录：

- current-safe 问题 ID、owner 和日期；
- 对 V1 P0/P1 闭环的影响；
- 权威输入及冲突处理；
- 可验证的决策标准与证据；
- 接收该问题的当前 primary prompt；
- open、decided、deferred 或 rejected 状态。

## 治理红线

- 问题处置不得修改既有 Authority Contract 或混用 HUMAN_KP / AI_KP。
- 不得用“尚未决定”为由允许业务层绕过 Agent Gateway 直连模型或 AI 直接写状态。
- Event Store、Visibility、Fact Provenance、Policy Gate 与审计边界不是可选问题。
- 历史问题清单不得作为当前阶段或 release 的 PASS 证据。

## Batch disposition / test responsibility

| 项目 | BATCH-051 处理 |
|---|---|
| Prompt coverage | P0014 |
| Role | documentation-or-traceability |
| Disposition | previous/provenance only；非当前 backlog 或验收入口 |
| 最小检查 | 核对 metadata、版权处置与顶层设计一致，确认未虚构当前决定 |
| 越界检查 | 确认未创建实现任务，未引入后续 batch Prompt ID 或历史实现提案 |
