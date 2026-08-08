---
document_id: CODEX-ROUTE-REPAIR
schema_version: 1
document_kind: mode-policy
mode: REPAIR
authority: governance-policy
status: ACTIVE
source_commit: "9940a9ff18ac4b09d27d982d84a0703d3469c781"
---

# REPAIR 模式

目标必须是当前计划分配的真实 milestone batch，或存在机器契约的 `GOV-*` governance maintenance。只读取原契约、source blocker、最新独立验收报告/问题 ID 和直接相关规范。仅修改获准范围；不得重规划里程碑、扩大范围、分配虚假 batch 或混入无关改进。
