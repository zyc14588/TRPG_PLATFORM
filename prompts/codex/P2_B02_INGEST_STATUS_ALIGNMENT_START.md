# Codex Prompt — P2 B02 Ingest Status / Storage Alignment Start

你是 Windows Codex App 中的 TRPG_PLATFORM repair agent。

批次：P2 B02 storage/status alignment repair。

目标：在正确的 B02 或 dedicated repair branch 中统一 `rag_core` 与 `storage` 的 ingest job status 语义，并修复 DB CHECK / migration / test harness 对齐问题。

本 session 允许修改 storage 和 migrations；不要修改 server/frontend/API/UI。

## 必须阅读

```text
CODEX_P2_MASTER_PROMPT.md
docs/p2/INDEX.md
docs/p2/04_STORAGE_RLS_DATABASE.md
docs/p2/19_INGEST_STATUS_SINGLE_SOURCE_POLICY.md
docs/status/P2_STATUS.md
crates/rag_core/src/lib.rs
crates/storage/src/lib.rs
migrations/**
```

## 允许修改

```text
crates/storage/**
migrations/**
docs/status/P2_STATUS.md
docs/p2/**
prompts/codex/**
```

## 禁止修改

```text
crates/server/**
apps/web/**
schemas/openapi.json
.env.example
```

除非编译必须，不要改 `crates/rag_core/**`。如果必须改 canonical enum，则同时更新 tests 和 status 文档。

## 任务

1. 收集状态：

```powershell
git status --short
git branch --show-current
git diff --stat
git diff --name-status
cargo metadata --no-deps
```

2. 检查平行状态模型：

```powershell
rg -n "enum .*IngestJobStatus|RagIngestJobStatus|IngestJobStatus|status.*denied|pending_review|claimed|parsing|embedding|indexed|failed" crates/rag_core crates/storage migrations docs
```

3. 以 `rag_core::IngestJobStatus` 为单一语义来源。不要在 storage 中保留独立语义 enum。

允许的兼容写法：

```rust
pub type RagIngestJobStatus = rag_core::IngestJobStatus;
```

或使用显式 SQL adapter，但 adapter 不得成为第二套语义模型。

4. 实现显式 DB string conversion。要求：

```text
- canonical status -> DB string 稳定；
- DB string -> canonical status 显式；
- unknown DB string 返回 error，不得 fallback；
- denied / pending_review / failed 等终态语义清楚。
```

5. 检查 migration CHECK 约束是否覆盖 canonical status set。如果缺少 `denied` 或其他 canonical value，新增 additive migration。不要修改旧 migration。

6. 修复 migration bootstrap / test harness 的 URL 边界：

```text
- migration/bootstrap/fresh install/rerun tests 使用 TRPG_TEST_MIGRATOR_DATABASE_URL 或 TRPG_DATABASE_ADMIN_URL；
- runtime repository/RLS proof 使用 ordinary app DATABASE_URL；
- 不要把 DATABASE_URL 改成 postgres；
- 不要给 trpg_app 授予 BYPASSRLS；
- 不要删除 ALTER ROLE trpg_app_private ... BYPASSRLS，除非 owner 明确改变安全设计。
```

7. 增加或更新测试：

```text
ingest_status_domain_db_values_are_aligned
denied_ingest_job_status_is_persistable
denied_license_is_not_indexed
migration_fresh_install_and_rerun_idempotence
ordinary_app_role_is_not_superuser_or_bypassrls
```

测试名可按项目风格调整，但必须覆盖这些不变量。

8. 更新 `docs/status/P2_STATUS.md`：

```text
- B02 status-model alignment completed / in progress；
- canonical enum source；
- DB CHECK migration name；
- command evidence；
- remaining blockers。
```

## 检查命令

先尝试加载本地 DB env：

```powershell
if (Test-Path .\scripts\dev\db\env.ps1) { . .\scripts\dev\db\env.ps1 }
Write-Host "DATABASE_URL=$env:DATABASE_URL"
Write-Host "TRPG_TEST_MIGRATOR_DATABASE_URL=$env:TRPG_TEST_MIGRATOR_DATABASE_URL"
Write-Host "TRPG_DATABASE_ADMIN_URL=$env:TRPG_DATABASE_ADMIN_URL"
```

然后运行：

```powershell
cargo fmt --all --check
cargo check --workspace
cargo test -p storage
```

如果有 migrator/admin URL：

```powershell
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
cargo sqlx prepare --check --workspace
cargo test --workspace
```

如果没有，不要伪造通过；最终报告写明 DB/migrator blocker。

## 最终报告格式

```markdown
## Batch summary
- Batch: P2 B02 ingest status/storage alignment
- Branch:
- Files changed:
- Canonical status source:
- Storage compatibility strategy:
- Migration names:
- Tests/checks run:
- Results:
- Environment blockers:
- Acceptance criteria met:
- Deferred items:
```
