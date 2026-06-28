# Codex Session Start — P2

在任何 P2 session 中先阅读：

1. `CODEX_P2_MASTER_PROMPT.md`
2. `docs/p2/INDEX.md`
3. `docs/p2/00_EXECUTION_RULES.md`
4. 当前批次对应文档
5. `prompts/codex/P2_CHECK_COMMANDS.md`

当前 P2 批次顺序：

```text
B00 docs install / prep gate
B01 rag_core domain model
B02 storage + PostgreSQL + RLS + database
B03 document_ingestor + worker
B04 Rig agent_engine
B05 server API + OpenAPI
B06 frontend UI
B07 hardening + final gate
```

禁止事项：

- 不要一次实现多个批次。
- 不要在实现 session 中做最终验收报告冒充独立验收。
- 不要在验收 session 中修改代码。
- 不要在 LocalOnly 路径调用 cloud provider。
- 不要让 UI/API handler 替代 storage/RLS/license/visibility 安全边界。
