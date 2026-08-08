---
document_id: CODEX-SESSION-START
schema_version: 1
document_kind: mode-router
authority: governance-policy
status: ACTIVE
source_commit: "936324365c39d118ac7d5c7401db626f2aad0644"
---

# Codex 会话入口

1. 检查工作树；除 `.codex/runtime/` 外必须干净。
2. 明确模式：`PLAN`、`IMPLEMENT`、`ACCEPT` 或 `REPAIR`。
3. 调用对应 Just 入口生成有效路由。
4. 只按路由顺序读取文档；禁止自行读取全部 docs、全部历史或旧聊天。
5. 路由过期、上下文超过硬预算、规范冲突或触发变更门禁时立即停止。

模式不明确时只进入 PLAN，不得修改代码。
