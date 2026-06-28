你是 TRPG_PLATFORM 仓库的独立验收审查 Codex，运行在 Windows Codex App 环境中。

本 session 只做 DB setup blocker 验收，不修改代码、文档、脚本或 lockfile。

验收目标：
确认 “No DATABASE_URL / No confirmed local PostgreSQL+pgvector target” 已被解决；migrations、SQLx prepare、storage DB tests、RLS proof 不再因为环境缺失被阻塞。

先运行：
1. `git status --short`
2. `git diff --stat`
3. `git diff --name-status`
4. `docker --version`
5. `docker compose version`
6. `cargo sqlx --version`
7. `Get-ChildItem Env:DATABASE_URL`
8. `Get-ChildItem Env:TRPG_DATABASE_ADMIN_URL`

必须检查文件：
- `docker-compose.dev-db.yml`
- `scripts/dev/db/start.ps1`
- `scripts/dev/db/env.ps1`
- `scripts/dev/db/grant-app-role.ps1`
- `scripts/dev/db/verify.ps1`
- `docs/p2/11_DATABASE_SETUP.md`
- `prompts/codex/P2_CHECK_COMMANDS.md`
- `.gitignore`
- `.env.example`

只读验收命令：
```powershell
docker compose -f docker-compose.dev-db.yml ps
. .\scripts\dev\db\env.ps1
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
cargo test -p storage
```

验收标准：
- `DATABASE_URL` 在当前 shell 中存在，且是非 superuser app role，例如 `trpg_app`。
- 有明确 PostgreSQL + pgvector target。
- `vector` extension 可用。
- migrations 成功运行。
- app role 不是 superuser、不是 BYPASSRLS。
- app role 能连接并访问必要 schema/table，但 RLS 仍启用/force。
- `cargo sqlx prepare --check --workspace` 成功，或失败原因是实际代码/query 问题，不再是 missing DB env。
- `cargo test -p storage` 成功，或失败原因是实际 storage/RLS 测试问题，不再是 missing DB env。
- `.env` 和本地密码没有被 Git 跟踪。
- 文档没有把 postgres superuser 推荐为 app runtime `DATABASE_URL`。

最终报告格式：

## 验收结论
- Result: PASS / CONDITIONAL PASS / FAIL / BLOCKED
- Batch: DB Setup Blocker Repair
- 当前分支:
- 变更范围摘要:

## DB target
- PostgreSQL host/port/db:
- pgvector:
- app role:
- migrator/admin role:
- DATABASE_URL present: yes/no, redacted

## 阻塞问题
- 每项包含文件路径、原因、影响、建议修复。

## 测试与命令
- 已运行命令及结果。
- 未运行命令及具体原因。

## RLS proof readiness
- 是否具备非 superuser ordinary role:
- 是否可以运行 DB-backed RLS tests:
- 是否仍有 blocker:

## 下一步
- 如果 PASS：返回 P2 B02/B07 DB-backed acceptance。
- 如果 FAIL/BLOCKED：列出最小 repair plan。
