# TRPG_PLATFORM P2 Documentation Index

本目录是 Codex P2 持久化阅读入口。任何 P2 实现或验收 session 都必须从这里开始。

## 阅读顺序

1. `../../CODEX_P2_MASTER_PROMPT.md` — P2 全局约束与完成定义。
2. `00_EXECUTION_RULES.md` — Codex session、分支、Windows 命令、验收纪律。
3. `01_SCOPE_ARCHITECTURE.md` — P2 范围、架构、crate 边界、数据流。
4. `02_BATCH_PLAN.md` — B00-B07 批次计划、入口/出口条件。
5. 当前批次 spec：
   - B01：`03_RAG_CORE_DOMAIN_MODEL.md`
   - B02：`04_STORAGE_RLS_DATABASE.md` 与 `11_DATABASE_SETUP.md`
   - B03：`05_INGEST_WORKER.md`
   - B04：`06_RIG_AGENT_ENGINE.md`
   - B05：`07_SERVER_API_OPENAPI.md`
   - B06：`08_FRONTEND_UI_UX.md`
   - B07：`09_SECURITY_LEGAL_PRIVACY_PROVIDER_POLICY.md` 与 `10_ACCEPTANCE_MATRIX.md`
6. `12_STATUS_REPORT_TEMPLATE.md` — P2 状态报告模板。
7. `13_CODEX_HANDOFF_TEMPLATE.md` — 每个 Codex session 的交接模板。

## 文档与提示词的边界

- `docs/p2/**` 是仓库内持久化设计文档，Codex 应长期读取。
- `prompts_for_user/**` 是用户复制给 Codex App 的启动/验收提示词，不一定放入仓库。
- `prompts/codex/P2_CHECK_COMMANDS.md` 是仓库内通用检查命令参考，可以被 Codex 读取。

## P2 批次图

```text
B00 Docs/Prep Gate
   ↓
B01 rag_core Domain Contracts
   ↓
B02 Storage + PostgreSQL + RLS + DB Setup
   ↓
B03 Document Ingestor + Worker
   ↓
B04 Rig Agent Engine
   ↓
B05 Server API + OpenAPI
   ↓
B06 Frontend RAG UI
   ↓
B07 Hardening + Final Gate
```

## 当前仓库状态文件

P2 进行中必须维护：

```text
docs/status/P2_STATUS.md
```

若文件不存在，请从 `docs/status/P2_STATUS_TEMPLATE.md` 或 `docs/p2/12_STATUS_REPORT_TEMPLATE.md` 创建。
