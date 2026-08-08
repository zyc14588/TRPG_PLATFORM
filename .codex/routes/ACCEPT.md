---
document_id: CODEX-ROUTE-ACCEPT
schema_version: 1
document_kind: mode-policy
mode: ACCEPT
authority: governance-policy
status: ACTIVE
source_commit: "9940a9ff18ac4b09d27d982d84a0703d3469c781"
---

# ACCEPT 模式

默认只读。目标可以是当前计划分配的真实 milestone batch，或存在机器契约的 `GOV-*` governance maintenance。按契约、要求、机器契约、Diff、证据、最后 Handoff 的顺序验收。不得直接修复或降低标准。输出 PASS、FAIL 或 BLOCKED，并生成稳定 ACC 问题 ID。
