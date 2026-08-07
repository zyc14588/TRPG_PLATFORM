---
document_id: CODEX-AUTONOMOUS-PLANNING
schema_version: 1
document_kind: governance-policy
authority: governance-policy
status: ACTIVE
source_commit: "1bb2e4ec9b569e4b8dab1e43ffb91e03c44f2acc"
---

# 自主规划政策

Codex 可拆分、合并、重排尚未开始批次；批次 ID 永不复用并保留墓碑。只有 PLAN 可改路线。默认顺序施工；只有 `parallel_safe` 且范围互斥、依赖独立时可并行。任何顶层、V1、公共契约、许可或门禁变化必须停止并提交变更提案。
