---
document_id: CODEX-SESSION-START
schema_version: 1
document_kind: mode-router
authority: governance-policy
status: ACTIVE
source_commit: "9940a9ff18ac4b09d27d982d84a0703d3469c781"
---

# Codex 会话入口

1. 检查工作树；除 `.codex/runtime/` 外必须干净。
2. 明确模式：`PLAN`、`IMPLEMENT`、`ACCEPT` 或 `REPAIR`；不新增第五种模式。
3. 调用对应入口生成有效路由。PLAN 使用 milestone target；IMPLEMENT 只使用真实 milestone batch；ACCEPT/REPAIR 使用真实 milestone batch 或已登记的 `GOV-*` governance-maintenance contract。
4. 只按路由顺序读取文档；禁止自行读取全部 docs、全部历史或旧聊天。
5. 路由过期、enforcement-enabled profile 超过硬预算、规范冲突或触发变更门禁时立即停止。Codex profile 的 context ratio 仅为 telemetry，不构成项目级停止门禁。

仓库外 Maintenance Bootstrap 只允许解除“控制面无法为自身生成合法 route”的一次性死锁；必须有明确授权、固定 scope 和审计记录，并在正式 maintenance REPAIR route 通过 `codex check` 后立即失效。

模式不明确时只进入 PLAN，不得修改代码。
