你是 TRPG_PLATFORM 仓库的独立验收 Codex，运行在 Windows / PowerShell 环境中。

本 session 只读验收：不要修改任何文件，不要运行会改写 tracked files 的命令。

批次：DB URL Scope Clean Repair acceptance。

验收目标：确认 `DATABASE_URL` empty blocker 已解除，并且本 DB env repair diff 没有混入 storage/server/frontend/RAG 业务逻辑变更。

必须阅读：
- `docs/p2/db/14_DB_URL_SCOPE_CLEAN_REPAIR.md`
- `docs/status/P2_DATABASE_STATUS.md`，如果存在
- `scripts/dev/db/env.ps1`
- `scripts/dev/db/verify.ps1`
- `.env.example`

首先运行：

```powershell
git status --short
git branch --show-current
git diff --stat
git diff --name-status
Get-ExecutionPolicy -List
```

范围验收：
- PASS 前提：diff 不包含 `crates/storage/**`、`crates/server/**`、`crates/rag_core/**`、`crates/document_ingestor/**`、`crates/worker/**`、`apps/web/**`、`migrations/**`、`schemas/**`、Cargo/frontend dependency manifests 或 lockfiles。
- 如果任何禁止文件仍在 diff 中，Result 必须是 FAIL。
- 如果禁止文件已被保存到仓库外 deferred patch，并已从当前 diff 移除，可继续验收。

DB URL 验收：

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev\db\env.ps1
if ([string]::IsNullOrWhiteSpace($env:DATABASE_URL)) { throw "DATABASE_URL is empty" }
if ($env:DATABASE_URL -match '://postgres(:|@)') { throw "DATABASE_URL uses postgres superuser" }
.\scripts\dev\db\verify.ps1
```

如果 `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass` 或 dot-source 被 MachinePolicy/UserPolicy 拦截：
- 不要把 DB URL 修复直接判 FAIL，除非脚本内容本身错误。
- 报告为 environment policy blocker。
- 可用静态审查和 manual env equivalent 判断脚本内容，但最终 Result 不能超过 CONDITIONAL PASS。

可选 DB-backed checks，只有本地 Docker/PostgreSQL 可用才运行：

```powershell
docker compose -f docker-compose.dev-db.yml up -d
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
```

最终报告必须使用：

## 验收结论
- Result: PASS / CONDITIONAL PASS / FAIL / BLOCKED
- Batch: DB URL Scope Clean Repair
- 当前分支:
- 变更范围摘要:

## 阻塞问题
- 每条包含文件路径、原因、影响、建议修复方式。

## 范围检查
- 是否存在禁止文件 diff。
- 是否 deferred storage/source_kind/harness patch。

## DATABASE_URL 检查
- 是否非空。
- 是否 ordinary app role。
- 是否拒绝 postgres superuser。

## PowerShell execution policy 检查
- 当前策略列表。
- process-scope bypass 是否可用。
- 是否仍有 PSSecurityException。

## 测试与命令
- 已运行命令及结果。
- 未运行命令及原因。

## 下一步
- 如果 PASS：进入 B02 storage/source_kind/harness 专用 repair。
- 如果 FAIL：列出最小 repair plan。
