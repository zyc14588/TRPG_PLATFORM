---
document_id: CODEX-ROUTE-PLAN
schema_version: 1
document_kind: mode-policy
mode: PLAN
authority: governance-policy
status: ACTIVE
source_commit: "7589256fc9f85fcd91e35d4593363ff5af9efe1b"
---

# PLAN 模式

允许读取当前项目摘要、里程碑出口、决策摘要、批次墓碑和阻塞。可创建或修改未开始路线和批次草案；不得修改业务代码、冻结批次或已完成记录。触及顶层设计时输出 CHANGE 提案。

当前计划 COMPLETE 后，可为紧邻的下一 V1 里程碑进入 PLAN；不得跳级或重规划已完成记录。下一里程碑尚未生成且缺少专用范围文档时，仅初始 PLAN 可读取现有路线图中的目标与出口来形成该文档。生成 ACTIVE 计划和所有真实批次路由仍要求专用范围与出口；计划切换仅在 PLAN 进行，并完整保留已完成计划。
