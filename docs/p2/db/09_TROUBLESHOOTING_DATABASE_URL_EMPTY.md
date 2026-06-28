# Troubleshooting Empty DATABASE_URL

## Symptom

`Get-ChildItem Env:DATABASE_URL` prints nothing, `cargo sqlx prepare --check --workspace` cannot find `DATABASE_URL`, or storage DB tests fail before connecting.

## Usual Cause

PowerShell scripts set variables only in their own process unless they are dot-sourced.

In a fresh Codex Windows shell, script execution policy may also block `.ps1` files before they run. If dot-sourcing fails with `PSSecurityException` or `UnauthorizedAccess`, enable only the current process first:

```powershell
Set-ExecutionPolicy -Scope Process Bypass -Force
```

If `Get-ChildItem Env:DATABASE_URL` throws `An item with the same key has already been added`, the Codex host process may have both `PATH` and `Path`. Normalize only the current process before rerunning the evidence command:

```powershell
$pathValue = [Environment]::GetEnvironmentVariable('PATH', 'Process')
[Environment]::SetEnvironmentVariable('Path', $null, 'Process')
[Environment]::SetEnvironmentVariable('PATH', $pathValue, 'Process')
```

Correct:

```powershell
. .\scripts\dev\db\env.ps1
```

Incorrect:

```powershell
.\scripts\dev\db\env.ps1
```

## Required Evidence

After dot-sourcing, this command must show all four variables:

```powershell
Get-ChildItem Env:DATABASE_URL, Env:TRPG_DATABASE_ADMIN_URL, Env:TRPG_TEST_MIGRATOR_DATABASE_URL, Env:TRPG_DATABASE_BOOTSTRAP_URL
```

`DATABASE_URL` must redact to:

```text
postgres://trpg_app:***@127.0.0.1:55432/trpg_platform
```

If `DATABASE_URL` is empty, do not switch it to `postgres`. Dot-source the env script in the same shell and rerun the DB gate.
