# Dot-source this file in PowerShell from the repository root:
#   . .\scripts\dev\db\env.ps1
#
# Local development only. Do not use these passwords in production.

$env:TRPG_DATABASE_ADMIN_URL = "postgres://postgres:postgres@127.0.0.1:55432/trpg_platform"
$env:TRPG_TEST_MIGRATOR_DATABASE_URL = $env:TRPG_DATABASE_ADMIN_URL
$env:TRPG_DATABASE_BOOTSTRAP_URL = "postgres://postgres:postgres@127.0.0.1:55432/postgres"
$env:DATABASE_URL = "postgres://trpg_app:trpg_app@127.0.0.1:55432/trpg_platform"
$env:TRPG_AUTH_MODE = "development"
$env:TRPG_AUTH_SECRET = "development-secret-at-least-32-bytes-change-me"
$env:TRPG_ALLOW_IN_MEMORY_STORE = "false"
$env:TRPG_CONFIG_PATH = "config/default.toml"
$env:TRPG_BIND_ADDR = "127.0.0.1:8080"
$env:NEXT_PUBLIC_API_BASE_URL = "http://127.0.0.1:8080"

Write-Host "DATABASE_URL set for current PowerShell process: postgres://trpg_app:***@127.0.0.1:55432/trpg_platform"
Write-Host "TRPG_DATABASE_ADMIN_URL set for migrations: postgres://postgres:***@127.0.0.1:55432/trpg_platform"
Write-Host "TRPG_TEST_MIGRATOR_DATABASE_URL set for storage migration tests: postgres://postgres:***@127.0.0.1:55432/trpg_platform"
