# BATCH-051 历史原型目录 Provenance

> 本页不是当前执行入口、实现计划、测试目录或验收入口。它只记录 P0013 的 current-safe 历史处置。

## Current-safe metadata

| Prompt ID | Prompt path | Source path | Source SHA-256 | Crate | Current-safe module | Current-safe output |
|---|---|---|---|---|---|---|
| CODEX-1084-99-APPENDIX-74cacc0ed6 | codex-prompts/99-appendix/P0013.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-minimal-rust-prototype-catalog-v4-strict-0546772399.v5-code-ready.md | f588d0706e7e57b475ed874e6f5aea788f0fa0cc325a297b32b0906e3f4d4ef0 | trpg-docs-governance | appendix_research::prototype_catalog_previousstrict | docs/codex/99-appendix/prototype_catalog_previous-provenance.md |

源路径、版本片段和 SHA 仅为 provenance；不得进入当前模块、测试、事件、指标或输出命名。

## Allowed boundary

- 仅记录历史材料的存在、处置和重新进入当前施工所需门禁。
- 不复制、排期、运行或验收历史原型任务。
- 不从本页创建代码、测试、迁移、运行时契约或依赖。

## Provenance disposition

| 历史材料类型 | Current-safe 处置 |
|---|---|
| 原型任务条目 | 仅保留来源证明，不构成当前 backlog |
| 历史 crate/module/output 建议 | 忽略；当前名称只从 current-safe maps 获取 |
| 版本、路径和 hash 派生名称 | 只允许出现在 metadata/provenance |
| 历史实现与测试提案 | 不在 BATCH-051 实施或复制 |
| 可能仍有价值的问题 | 必须由当前 stage/batch 重新提出，并绑定 current-safe primary prompt 与验收证据 |

## 治理红线

- 历史原型不能绕过 Authority Contract、`HUMAN_KP` / `AI_KP` 互斥或 Agent Gateway。
- 历史材料不能授权 AI 直写正式状态或让读模型成为正史。
- Event Store 仍是正史，任何原型均不得另建正式事实来源。
- Visibility、Fact Provenance、Policy Gate 和审计要求不能以“原型”为由省略。
- 本页不得被引用为当前 V1 PASS、阶段 PASS 或 release PASS 的证据。

## Batch disposition / test responsibility

| 项目 | BATCH-051 处理 |
|---|---|
| Prompt coverage | P0013 |
| Role | documentation-or-traceability |
| Disposition | previous/provenance only；非当前执行或验收入口 |
| 最小检查 | 核对 metadata 与 current-safe maps，确认标题和正文均明确 provenance 状态 |
| 越界检查 | 确认没有历史任务被排期、没有实现产物、没有后续 batch Prompt ID |
