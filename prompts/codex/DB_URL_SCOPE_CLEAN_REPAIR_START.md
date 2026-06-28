你是 TRPG_PLATFORM 仓库的 Codex，运行在 Windows / PowerShell 环境中。

任务：DB URL scope clean repair。

背景：`DATABASE_URL` empty blocker 已基本解除，但当前验收 FAIL，因为 DB env repair 分支混入了 `crates/storage/src/lib.rs` 的业务逻辑 diff，例如 `source_kind_as_str/source_kind_from_str` 和 storage DB test URL harness 改动。本 session 只做 DB env-only 范围清理，不继续改 storage。

必须先阅读：
- `CODEX_DB_MASTER_PROMPT.md`，如果存在
- `.codex/DB_SESSION_START.md`，如果存在
- `docs/p2/db/14_DB_URL_SCOPE_CLEAN_REPAIR.md`
- `docs/p2/db/02_DATABASE_URL_CONTRACT.md`，如果存在
- `docs/p2/db/09_TROUBLESHOOTING_DATABASE_URL_EMPTY.md`，如果存在
- `docs/status/P2_DATABASE_STATUS.md`，如果存在

开始前运行：

```powershell
git status --short
git branch --show-current
git diff --stat
git diff --name-status
Get-ExecutionPolicy -List
```

允许修改：
- `.env.example`
- `docker-compose.dev-db.yml`
- `scripts/dev/db/*.ps1`
- `docs/p2/db/**`
- `docs/status/P2_DATABASE_STATUS.md`
- `docs/status/P2_DATABASE_STATUS_TEMPLATE.md`
- `CODEX_DB_MASTER_PROMPT.md`
- `.codex/DB_SESSION_START.md`
- `prompts/codex/DB_*.md`
- `README.md` 中的 DB reading order 链接

禁止修改：
- `crates/storage/**`
- `crates/server/**`
- `crates/rag_core/**`
- `crates/document_ingestor/**`
- `crates/worker/**`
- `apps/web/**`
- `migrations/**`
- `schemas/**`
- Cargo / frontend dependency manifests 和 lockfiles

如果 `crates/storage/src/lib.rs` 或其他禁止文件已有 tracked 修改：
1. 先把 worktree 和 index diff 保存到仓库外 `..\_trpg_codex_deferred\`。
2. 文件名包含时间戳和 B02 用途，例如 `20260629_153000_B02_storage_worktree.patch`。
3. 然后从当前 DB env repair 工作区恢复这些禁止文件。
4. 不要把 deferred patch 加进仓库。
5. 在 `docs/status/P2_DATABASE_STATUS.md` 记录 deferred patch 路径和原因。

PowerShell execution policy 处理规则：
- 不要修改 CurrentUser 或 LocalMachine execution policy。
- 可以在当前 session 使用：

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev\db\env.ps1
```

- 如果仍出现 PSSecurityException，记录为机器/用户策略 blocker；不要把它当成 `.env.example` 或 `env.ps1` 内容失败。
- 可以用手动 `$env:` 赋值继续验证 DB URL contract，但必须报告这一点。

必须确认：
- `DATABASE_URL` 非空。
- `DATABASE_URL` 不是 `postgres://postgres...`。
- `.env.example` 与 `scripts/dev/db/env.ps1` 是有效多行格式。
- `verify.ps1` 能 fail-fast 拒绝空 `DATABASE_URL` 和 postgres superuser app URL。
- 当前最终 diff 不包含禁止文件。

建议检查：

```powershell
git diff --name-status

$forbidden = git diff --name-only | Where-Object { $_ -match '^(crates/storage|crates/server|crates/rag_core|crates/document_ingestor|crates/worker|apps/web|migrations|schemas)/' }
if ($forbidden) { $forbidden; throw "Out-of-scope files remain in DB env repair diff" }

Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
. .\scripts\dev\db\env.ps1
if ([string]::IsNullOrWhiteSpace($env:DATABASE_URL)) { throw "DATABASE_URL is empty" }
if ($env:DATABASE_URL -match '://postgres(:|@)') { throw "DATABASE_URL uses postgres superuser" }
.\scripts\dev\db\verify.ps1

git diff --check
```

如果本地 Docker/PostgreSQL 可用，再运行：

```powershell
docker compose -f docker-compose.dev-db.yml up -d
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
```

最终报告格式：

## Batch summary
- Batch: DB URL Scope Clean Repair
- Files changed:
- Out-of-scope files removed from diff:
- Deferred patch paths:
- DATABASE_URL result:
- PowerShell execution policy result:
- Commands run:
- Results:
- Remaining blockers:
- Next repair branch:
