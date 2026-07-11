# BATCH-051 日期化技术研究说明（2026-06-30）

> 本页记录 2026-06-30 材料中的技术决策背景，不声称这些比较或外部事实在当前日期仍是最新。任何易变事实必须在使用时通过官方资料重新核验并记录核验日期。

## Current-safe metadata

| Prompt ID | Prompt path | Source path | Source SHA-256 | Crate | Current-safe module | Current-safe output |
|---|---|---|---|---|---|---|
| CODEX-1087-99-APPENDIX-504bf177bd | codex-prompts/99-appendix/P0016.md | docs/implementation/90-traceability/per-file-code-ready/99-appendix/docs-implementation-99-appendix-research-notes-2026-06-30-46eb7a14ef.v5-code-ready.md | d56f7157295b3827efc5c234802fec7d1c948accd5bc6485b255e145c19436ef | trpg-docs-governance | appendix_research::research_notes_2026_06_30 | docs/codex/99-appendix/research_notes_2026_06_30.md |

源路径、版本片段和 SHA 只用于 provenance。

## Allowed boundary

- 只记录日期化研究背景、当时采用的施工基线和重新核验要求。
- 本页不新增依赖、不批准替换技术栈、不创建产品实现。
- 当前施工仍以顶层设计、持久上下文和 current-safe primary prompts 为准。

## 治理红线

- 技术选择不得改变 Authority Contract、`HUMAN_KP` / `AI_KP` 互斥或正式写入链路。
- 模型与工具接入不得绕过 Agent Gateway、Tool Permission Gate 或 Policy Gate。
- Event Store 仍是正史；消息系统、缓存、Projection 和向量索引均不能替代正史。
- Visibility Label 与 Fact Provenance 必须贯穿检索、日志、指标、导出和回放。
- 本地模型不得静默 fallback 到云端；跨隐私边界必须显式配置并审计。

## 2026-06-30 记录的施工基线

| 领域 | 日期化记录 | 使用限制 |
|---|---|---|
| Rust 数据访问 | SQLx 作为仓库施工基线 | 这是项目选择，不是对当前生态“最新/最佳”的声明 |
| 事件传递 | NATS JetStream 作为 Event Store 派生消息的 adapter | 消息流不是正史，不能先发布后补正式事件 |
| 授权与策略 | OpenFGA 处理关系授权，OPA 处理上下文策略 | 两层门禁都不能被 Agent 或插件绕过 |
| RAG | PostgreSQL/pgvector 承载可重建检索读模型 | 索引、chunk 和 summary 不能成为 confirmed fact 来源 |
| Workflow | 记录为内部实现优先，外部工作流系统仅保留 adapter 可能性 | 本页不批准新增外部依赖或扩大 V1 scope |
| API 与观测 | Axum/utoipa、tracing/OpenTelemetry 属于施工基线 | 不在本页声明具体版本、兼容性或最新能力 |

## 后续核验规则

1. 易变事实只使用官方文档、官方仓库或项目认可的第一方资料。
2. 每条外部结论记录来源、访问日期、适用版本和已知限制。
3. 明确区分“项目已采用的基线”和“外部工具当前能力”。
4. 新研究若改变既有基线，必须经过 change control，并由 current-safe primary prompt 承接。
5. 未实际核验的内容标记为待核验，不得写成 PASS 或“最新”。

## Batch disposition / test responsibility

| 项目 | BATCH-051 处理 |
|---|---|
| Prompt coverage | P0016 |
| Role | documentation-or-traceability |
| Scope | 日期化研究说明；未做实时版本调查，未生成实现产物 |
| 最小检查 | 核对 metadata 与 current-safe maps；确认所有易变表述都有日期/限制 |
| 红线检查 | 确认技术说明不改变正史、AI、Visibility、Provenance、Policy 或 fallback 边界 |
| 越界检查 | 确认未声称最新、未批准依赖、未复制历史实现提案或后续 batch metadata |
