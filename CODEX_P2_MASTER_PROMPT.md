# TRPG_PLATFORM — Codex P2 Master Prompt

本文件是 Phase 2 的最高优先级执行文档。Codex 在处理任何 P2 任务前必须先阅读本文件，再阅读 `docs/p2/INDEX.md` 和对应批次文档。

## P2 一句话目标

P2 交付一个面向 TRPG room 的 Rules / RAG / Document Ingestion / Rig Agent Engine foundation：允许用户安全导入可用文本，确定性 chunk，持久化 provenance，经过 PostgreSQL/RLS/license/visibility gate 检索证据，并用 Rig 作为 Rust agent/provider orchestration engine 暴露可控、可审计、可测试的 evidence-first workflow。

## P2 明确不做

- 不导入未经授权的商业规则原文。
- 不把 UI 或 API handler 当作唯一安全边界。
- 不让 `pending_review`、`denied`、KP-only、SystemInternal 内容进入 ordinary retrieval candidate set。
- 不在 LocalOnly room 调用 cloud LLM、cloud embedding、cloud rerank、cloud OCR 或 image provider。
- 不在 P2 生成最终剧情/GM 回答；P2 的默认 query/agent 输出是 evidence、citations、provenance、applied filters 和 provider metadata。
- 不在测试中调用真实云 provider 或真实付费服务。
- 不把 API key、DB URL、JWT secret、hidden content 写进 docs、examples、OpenAPI examples、snapshots、logs 或 metrics。

## Rig 在 P2 中的角色

Rig 是 agent/provider orchestration engine，不是安全边界。P2 中 `crates/agent_engine` 应封装 Rig provider、completion、embedding、tool-call 和 workflow adapter；但 license gate、visibility gate、room membership、RLS、idempotency、DTO safety 必须在 `rag_core`、`storage`、`document_ingestor` 和 `server` 层强制。

推荐 crate 边界：

```text
crates/rag_core            # 领域类型、traits、错误、不变量；无 DB/HTTP/env/cloud secret
crates/storage             # PostgreSQL schema, SQLx, RLS, repository, idempotency
crates/document_ingestor   # deterministic ingest orchestration, license-first
crates/worker              # optional job runner; no HTTP/UI
crates/agent_engine        # Rig-backed provider/agent adapters; no RLS bypass
crates/server              # HTTP routes, auth, CSRF, OpenAPI, DTO mapping
apps/web                   # UI surface only; never a security boundary
```

## 全局强制不变量

1. License gate 在 chunking、embedding、indexing、retrieval、agent tool-call 之前执行。
2. Visibility gate 在 vector/keyword scoring、ranking、reranking、prompt construction 之前执行。
3. ordinary DB role / ordinary retrieval path 对 `pending_review` 与 `denied` DB-deny-by-default。
4. KP-only / GM-only / SystemInternal 内容不能进入 PL/observer retrieval candidate set。
5. 每条 evidence 必须包含 source/document/chunk identity、content_hash、citation/location、safe visibility metadata、provider metadata。
6. `top_k`、upload size、raw text size、chunk size、prompt/evidence token budget 必须有硬上限。
7. Ingest 必须 idempotent：同 key + 同 payload replay；同 key + 不同 payload conflict。
8. 所有 RLS/security tests 必须使用 ordinary app role 或项目等价低权限 role；不能用 `postgres` superuser 证明安全。
9. API 返回 DTO，不泄露 raw DB rows 或 hidden existence。
10. Rig 工具调用只能调用 policy-guarded repository/service；不得直接读 DB 绕过 repository/RLS。

## 批次纪律

每个批次必须独立分支、独立 Codex session、独立验收 session。禁止跨批次偷跑：

- B01 Domain 不写 DB migration、server route、frontend UI。
- B02 Storage/RLS 不暴露 public HTTP route，不写 frontend。
- B03 Ingest Worker 不暴露 API/UI，不接 cloud live provider。
- B04 Rig Agent Engine 不绕过 storage/RLS，不做 final answer UX。
- B05 Server API 不写 frontend pages，不把 generated final answer 作为 P2 交付。
- B06 Frontend UI 不改变 backend security semantics。
- B07 Hardening 只做测试、docs、小 bug fix，不扩产品范围。

## Codex 工作方式

- 先读文档，再改代码。
- 优先小 patch，避免大范围重构。
- 不通过削弱断言“修复”测试。
- 不自动升级依赖，除非当前批次明确需要并记录理由。
- Windows 环境优先使用 PowerShell 兼容命令。
- DB、Docker、E2E 缺失时继续静态审查和非 DB 检查，但最终报告必须标明未运行命令与原因；不能伪造 PASS。

## P2 Done Definition

P2 完成必须满足：

- `docs/p2/10_ACCEPTANCE_MATRIX.md` 中每个必需项都有 automated test、SQLx proof、OpenAPI proof 或明确 deferred rationale。
- `docs/status/P2_STATUS.md` 存在且不把未实现项写成已完成。
- Rust workspace、SQLx/migrations、frontend gates、OpenAPI/schema checks 在可用环境中通过。
- 无 tracked generated artifacts、无 secret leakage、无 license/RLS/visibility/auth blocker。
