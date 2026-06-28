你是正在 Windows 环境中处理 TRPG_PLATFORM 仓库的 Codex。

批次：DB Migrator URL Test Repair — 修复 storage migration bootstrap 使用错误 role 的测试问题。

背景：
当前 DB 环境已经不再是 “No DATABASE_URL”。剩余失败是：
`migrations/20260626021000_p1_5_auth_private_and_rag_license_rls.sql:6` 中的 `ALTER ROLE trpg_app_private ... BYPASSRLS` 在 `cargo test -p storage` 的 fresh migration bootstrap 中通过普通 app-role-derived `DATABASE_URL` 执行，导致权限失败。

目标：
修复测试/DB bootstrap harness，使“运行迁移/role bootstrap”的连接使用 admin/migrator URL，而应用 runtime、repository、SQLx prepare、RLS proof 仍使用普通 app role 的 `DATABASE_URL`。不要为了让测试通过把 `DATABASE_URL` 改成 postgres。

必须保持的边界：
- `DATABASE_URL` = ordinary app role，例如 `postgres://trpg_app:...`。用于 app runtime、repository tests、RLS proof、SQLx prepare after migrations。
- `TRPG_DATABASE_ADMIN_URL` 或 `TRPG_TEST_MIGRATOR_DATABASE_URL` = admin/migrator role。仅用于 migration/bootstrap/extension/role setup/fresh DB migration tests。
- ordinary app role 不得是 superuser，不得拥有 BYPASSRLS。
- 不得删除或弱化 migration 里的 `ALTER ROLE trpg_app_private ... BYPASSRLS`，除非有完全等价且更安全的迁移语义。

首先运行：
```powershell
git status --short
git branch --show-current
cargo metadata --no-deps
rg -n "BYPASSRLS|ALTER ROLE|TRPG_DATABASE_ADMIN_URL|TRPG_TEST_MIGRATOR_DATABASE_URL|DATABASE_URL|sqlx::migrate|migrate!|Migrator|run\(" migrations crates/storage scripts docs .env.example
```
如果 `rg` 不可用，使用 PowerShell `Select-String`。

必须阅读：
- `migrations/20260626021000_p1_5_auth_private_and_rag_license_rls.sql`
- `crates/storage/**` 中所有 migration/bootstrap/integration test helper
- `scripts/dev/db/env.ps1`
- `scripts/dev/db/verify.ps1`
- `scripts/dev/db/grant-app-role.ps1`
- `.env.example`
- `docs/p2/11_DATABASE_SETUP.md`
- 如果已复制：`docs/p2/15_DB_TEST_MIGRATOR_POLICY.md`

任务：
1. 找到 `cargo test -p storage` 中运行 fresh migration bootstrap 的测试或 helper。
2. 将“运行 migrations / role bootstrap / extension setup / create fresh test DB”的连接改为读取：
   - 优先 `TRPG_TEST_MIGRATOR_DATABASE_URL`
   - 其次 `TRPG_DATABASE_ADMIN_URL`
   - 不得默认使用 app `DATABASE_URL` 执行 privileged migrations。
3. 将 repository/RLS proof 的连接继续保持为 `DATABASE_URL`，并加 guard 或测试证明它不是 `postgres` superuser。
4. 如已有 test helper 名称与上面不同，使用项目现有风格实现等价 helper，不要引入无关大重构。
5. 确认 `.env.example` 和 `scripts/dev/db/env.ps1` 明确暴露两个 URL：
   - admin/migrator URL
   - ordinary app `DATABASE_URL`
6. 更新 `docs/p2/11_DATABASE_SETUP.md` 或新增/引用 `docs/p2/15_DB_TEST_MIGRATOR_POLICY.md`，写清 migration tests 的 URL split。
7. 不要把 runtime `DATABASE_URL` 改成 postgres。
8. 不要跳过/ignore 掉关键 migration-idempotence 或 RLS 测试来制造绿色结果。若某个测试确实需要 admin URL且当前 shell 缺失，应给出明确错误或前置说明；在已有脚本提供 admin URL的环境下必须可运行。

建议实现形态：
- 在 storage integration tests 中增加 helper：`migrator_database_url()` 与 `app_database_url()`，或项目等价函数。
- migration bootstrap 使用 `migrator_database_url()`。
- app repository pool/RLS test pool 使用 `app_database_url()`。
- 如测试创建临时 DB，创建/迁移/role grants 用 migrator URL；验证 RLS 查询用 app URL。

必须运行：
```powershell
. .\scripts\dev\db\env.ps1

Write-Host "DATABASE_URL=$($env:DATABASE_URL)"
Write-Host "TRPG_DATABASE_ADMIN_URL=$($env:TRPG_DATABASE_ADMIN_URL)"

cargo fmt --all --check
cargo check --workspace
cargo test -p storage
cargo sqlx migrate run --database-url "$env:TRPG_DATABASE_ADMIN_URL"
.\scripts\dev\db\grant-app-role.ps1
.\scripts\dev\db\verify.ps1
cargo sqlx prepare --check --workspace
```
如果 DB 或 Docker 未运行，先不要伪造通过；报告环境 blocker。但如果 admin URL 和 app URL 都存在，必须解决 `ALTER ROLE ... BYPASSRLS` 权限失败。

最终报告格式：

## Batch summary
- Batch: DB Migrator URL Test Repair
- Files changed:
- Root cause fixed:
- Migrator/admin URL behavior:
- App/RLS URL behavior:
- Tests/checks run:
- Results:
- Remaining failures:
- Commands that could not run and why:
- Next recommended acceptance prompt:
