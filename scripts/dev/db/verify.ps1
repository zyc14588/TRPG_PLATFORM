$ErrorActionPreference = "Stop"

if (-not $env:DATABASE_URL) {
  throw "DATABASE_URL is not set. Run: . .\scripts\dev\db\env.ps1"
}

if ($env:DATABASE_URL -match "^postgres(ql)?://postgres(:|@)") {
  throw "DATABASE_URL must use the non-superuser app role, not postgres."
}

$container = "trpg-platform-pgvector"

function Invoke-Docker {
  docker @args
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

Write-Host "Checking pgvector extension..."
Invoke-Docker exec $container psql -U postgres -d trpg_platform -v ON_ERROR_STOP=1 -c "CREATE EXTENSION IF NOT EXISTS vector;"
Invoke-Docker exec $container psql -U postgres -d trpg_platform -v ON_ERROR_STOP=1 -c "SELECT extname, extversion FROM pg_extension WHERE extname IN ('vector', 'pgcrypto');"

Write-Host "Checking non-superuser app role..."
Invoke-Docker exec $container psql -U postgres -d trpg_platform -v ON_ERROR_STOP=1 -c "SELECT rolname, rolsuper, rolbypassrls, rolcreatedb FROM pg_roles WHERE rolname = 'trpg_app';"

Write-Host "Checking connection as trpg_app..."
Invoke-Docker exec -e PGPASSWORD=trpg_app $container psql -h 127.0.0.1 -p 5432 -U trpg_app -d trpg_platform -v ON_ERROR_STOP=1 -c "SELECT current_user;"

Write-Host "Checking RLS proof role..."
Invoke-Docker exec $container psql -U postgres -d trpg_platform -v ON_ERROR_STOP=1 -c "SELECT rolname, rolsuper, rolbypassrls, rolcanlogin, rolcreatedb FROM pg_roles WHERE rolname IN ('trpg_app', 'trpg_rls_test') ORDER BY rolname;"
Invoke-Docker exec $container psql -U postgres -d trpg_platform -v ON_ERROR_STOP=1 -c "SELECT pg_has_role('trpg_app', 'trpg_app_private', 'member') AS trpg_app_can_set_private_role;"

Write-Host "Checking RLS-enabled tables..."
Invoke-Docker exec $container psql -U postgres -d trpg_platform -v ON_ERROR_STOP=1 -c "SELECT relname, relrowsecurity, relforcerowsecurity FROM pg_class WHERE relname IN ('documents','chunks','document_sources','ingest_jobs') ORDER BY relname;"

Write-Host "DB verification complete."
