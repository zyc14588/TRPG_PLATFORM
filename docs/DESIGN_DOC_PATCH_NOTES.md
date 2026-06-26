# Design Document Patch Notes for TRPG_PLATFORM

这些补丁说明用于修复当前设计文档组中的误导点，并让 Codex 能高效执行 P2。

## 必改文件

### `docs/status/P1_AUDIT.md`

当前问题：文件声称 RAG kernel 已经实现，但实际代码未实现。

建议替换结论：

```text
P1 provides a valid foundation, but P1.5 stabilization is required before P2.
The RAG kernel remains a skeleton. The previously listed RAG fixes are P2 tasks, not completed P1 fixes.
```

删除或改写：

- `Fixes Applied in This Pass`
- 所有“已扩展 rag_core”表述
- 所有“document_ingestor 已委托 rag_core”表述
- 所有“prompts/03 已扩展”为事实的表述

### `prompts/02_REALTIME_CONCURRENCY.md`

历史问题：realtime prompt 曾把 Realtime and Concurrency 标为主线 Phase 2；当前应保持为 Phase 3 或 P2B。

建议标题：

```text
# Codex Phase 3 — Realtime and Concurrency (Deferred; Not Current P2 Mainline)
```

或：

```text
# Codex P2B — Realtime and Concurrency, deferred until Rules/RAG P2 is complete
```

### `prompts/03_RULES_RAG.md`

历史问题：标题曾为 Phase 3，内容过短。当前应保持为 P2 Rules/RAG/Document Ingestion 的边界说明。

建议替换为 `P2_CODEX_HANDOFF.md` 中的主任务说明，或至少包含：

- P2 唯一目标
- 禁止事项
- crate 边界
- license gate
- visibility policy
- stop points
- acceptance tests

### `docs/RAG_DESIGN.md`

当前问题：它描述的是目标设计，但措辞像当前实现。

建议增加：

```text
Implementation status: target design. The current P1 codebase does not yet implement all types listed here.
```

并把 `Core types live in crates/rag_core` 改为：

```text
P2 must implement these core types in crates/rag_core.
```

### `docs/P2_DELIVERY_PLAN.md`

当前问题：范围正确，但没有说明 P1.5 gate。

建议增加：

```text
P2 may start only after P1.5 stabilization fixes production startup, idempotency transaction boundaries, refresh rotation atomicity, auth-private-table RLS strategy, and license-status RLS enforcement.
```

### `docs/SECURITY_RLS_POLICY.md`

必须明确：

- 生产 app role 不得是 postgres 超级用户。
- 认证私有表使用 BYPASSRLS app-private role 还是 SECURITY DEFINER functions。
- RAG retrieval 普通路径必须强制 `license_status='allowed'`。
- pending/denied 只进入 review path。

## 建议新增文件

- `docs/P1_5_STABILIZATION_PLAN.md`
- `docs/P2_RAG_IMPLEMENTATION_SPEC.md`
- `docs/P2_RAG_ACCEPTANCE_TESTS.md`
- `docs/status/P1_AUDIT_CORRECTED.md`

## 建议 CODEX_MASTER_PROMPT 增加的门禁语句

```text
Current active phase: P1.5 stabilization, then P2 Rules/RAG. Do not implement realtime/websocket/agent work until P2 Rules/RAG acceptance tests pass.
```

P1.5 完成后改为：

```text
Current active phase: P2 Rules/RAG/Ingestion. Realtime/WebSocket/Redis work is deferred.
```
