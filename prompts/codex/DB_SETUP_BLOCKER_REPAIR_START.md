你是正在 Windows 环境中处理 TRPG_PLATFORM 仓库的 Codex。

批次：DB Setup Blocker Repair — PostgreSQL / pgvector / SQLx / RLS proof environment。

目标：
把当前 “No DATABASE_URL / No confirmed local PostgreSQL+pgvector target” 从环境 blocker 修成可复现、可验收的本地 DB setup。不要实现 P2 runtime 功能；只允许新增/修正文档、dev-only docker compose、PowerShell 脚本、.gitignore/env 示例和必要的 DB setup 说明。

必须遵守：
- 优先使用 Windows PowerShell。
- 不要提交真实 secret。
- `.env`、`.env.local`、`.env.local.db` 必须保持 untracked。
- `postgres` superuser 只能用于本地 migration/bootstrap；普通 app runtime/RLS proof 必须使用非 superuser，例如 `trpg_app`。
- 如果 Docker 不可用，不要伪造 PASS；输出 blocker 和替代方案。

先运行：
1. `git status --short`
2. `git branch --show-current`
3. `docker --version`
4. `docker compose version`
5. `cargo metadata --no-deps`
6. `cargo sqlx --version`；如果不可用，记录并安装建议，不要盲目修改 Rust toolchain。

必须检查：
- `.env.example`
- `.gitignore`
- `docs/p2/11_DATABASE_SETUP.md`
- `migrations/**`
- `crates/storage/**`
- `prompts/codex/P2_CHECK_COMMANDS.md`

任务：
1. 确认仓库有 dev-only `docker-compose.dev-db.yml`，使用 PostgreSQL + pgvector，并避免占用本机 5432；推荐映射到 55432。
2. 确认或新增 `scripts/dev/db/start.ps1`、`scripts/dev/db/env.ps1`、`scripts/dev/db/grant-app-role.ps1`、`scripts/dev/db/verify.ps1`。
3. 确认 `.gitignore` 覆盖 `.env`、`.env.local`、`.env.local.db` 等本地环境文件。
4. 更新 `docs/p2/11_DATABASE_SETUP.md`，写清：
   - 为什么没有 `DATABASE_URL` 必须 BLOCKED；
   - admin/migrator URL 与 ordinary app `DATABASE_URL` 的区别；
   - migration、grant、verify、SQLx prepare、storage tests 的顺序；
   - RLS proof 不得使用 postgres superuser。
5. 更新 `prompts/codex/P2_CHECK_COMMANDS.md` 或等价检查文档，把 DB setup 命令加入 P2 B02/B07 gate。
6. 如果当前 shell 可以运行 Docker：
   - `docker compose -f docker-compose.dev-db.yml up -d`
   - dot-source env 脚本：`. .\scripts\dev\db\env.ps1`
   - `cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"`
   - `.\scripts\dev\db\grant-app-role.ps1`
   - `.\scripts\dev\db\verify.ps1`
   - `cargo sqlx prepare --check --workspace`
   - `cargo test -p storage`
7. 如果 Docker/SQLx/PostgreSQL 不可用，仍提交文档和脚本，但最终报告必须标记环境 blocker，不得声称 DB proof 已通过。

不要做：
- 不要修改 P2 domain/storage/server/frontend runtime 逻辑，除非只是修文档引用。
- 不要改 migrations 来绕过权限或 RLS。
- 不要把 `DATABASE_URL=postgres://postgres:...` 写成 app runtime 默认。
- 不要把 DB unavailable 写成 PASS。

最终报告格式：

## Batch summary
- Batch: DB Setup Blocker Repair
- Files changed:
- Local DB target:
- Environment variables:
- Commands run:
- Results:
- Commands blocked and exact reason:
- RLS proof role:
- Next recommended acceptance prompt:
