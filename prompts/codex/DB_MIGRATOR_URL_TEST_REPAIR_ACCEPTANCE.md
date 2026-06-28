你是 TRPG_PLATFORM 仓库的独立验收审查 Codex，运行在 Windows Codex App 环境中。

本 session 只做验收，不做实现。不要修改源代码、测试、迁移、文档或 lockfile。

批次：DB Migrator URL Test Repair Acceptance。

验收目标：确认 `cargo test -p storage` 不再因为 fresh migration bootstrap 用普通 app-role `DATABASE_URL` 执行 `ALTER ROLE ... BYPASSRLS` 而失败；同时确认 app runtime / repository / RLS proof 仍使用普通 app role，不把 `DATABASE_URL` 改成 postgres。

先收集状态：
```powershell
git status --short
git branch --show-current
git diff --stat
git diff --name-status
cargo metadata --no-deps
rg -n "BYPASSRLS|ALTER ROLE|TRPG_DATABASE_ADMIN_URL|TRPG_TEST_MIGRATOR_DATABASE_URL|DATABASE_URL|sqlx::migrate|migrate!|Migrator" migrations crates/storage scripts docs .env.example
```

必须检查：
1. `DATABASE_URL` 仍是 ordinary app role，例如 `trpg_app`；不得是 `postgres` superuser。
2. migration/bootstrap/fresh DB test harness 使用 `TRPG_TEST_MIGRATOR_DATABASE_URL` 或 `TRPG_DATABASE_ADMIN_URL`。
3. repository/RLS proof 使用 `DATABASE_URL`。
4. ordinary app role 没有被授予 `BYPASSRLS`。
5. migration `20260626021000_p1_5_auth_private_and_rag_license_rls.sql` 的 privileged role setup 没有被删除或弱化成静默跳过。
6. 没有通过 `#[ignore]`、删除测试、削弱断言来掩盖失败。
7. docs/scripts 明确记录 two-URL policy。

运行检查：
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

判定：
- PASS：storage tests 通过，且 `ALTER ROLE ... BYPASSRLS` 不再通过 ordinary app role 执行；`DATABASE_URL` 仍是 non-superuser app role；RLS proof 未被削弱。
- FAIL：仍然用 `DATABASE_URL` 跑 privileged migrations；或把 `DATABASE_URL` 改成 postgres；或删除/跳过关键测试；或普通 app role 获得 BYPASSRLS；或 migration 语义被弱化。
- BLOCKED：DB/Docker/admin URL 仍不存在，导致无法运行 DB-backed proof。不得把 BLOCKED 说成 PASS。

最终报告必须使用：

## 验收结论
- Result: PASS / FAIL / BLOCKED
- Batch: DB Migrator URL Test Repair Acceptance
- 当前分支:
- 变更范围摘要:

## 阻塞问题
- 文件路径、原因、影响、建议修复方式。

## URL / role boundary
- DATABASE_URL role:
- Admin/migrator URL source:
- RLS proof role:
- ordinary app role has BYPASSRLS: yes/no/unknown

## 测试与命令
- 已运行命令及结果。
- 未运行命令及原因。

## 安全结论
- 是否保持 app runtime 非 superuser。
- 是否保持 RLS proof 有效。
- 是否避免迁移语义弱化。

## 下一步
- 如果 PASS：回到 P2 B02/B07 DB-backed acceptance。
- 如果 FAIL/BLOCKED：列出最小 repair plan。
