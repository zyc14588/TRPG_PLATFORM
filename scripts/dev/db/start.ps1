$ErrorActionPreference = "Stop"

$container = "trpg-platform-pgvector"

function Invoke-Docker {
  docker @args
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

Write-Host "Starting local PostgreSQL + pgvector container..."
Invoke-Docker compose -f docker-compose.dev-db.yml up -d

Write-Host "Waiting for PostgreSQL health..."
$deadline = (Get-Date).AddSeconds(90)
do {
  $status = docker inspect --format='{{.State.Health.Status}}' $container 2>$null
  if ($status -eq "healthy") {
    break
  }
  Start-Sleep -Seconds 2
} while ((Get-Date) -lt $deadline)

$status = docker inspect --format='{{.State.Health.Status}}' $container
if ($status -ne "healthy") {
  throw "PostgreSQL container did not become healthy. Current status: $status"
}

Write-Host "Creating local app role if needed..."
Invoke-Docker exec $container psql -U postgres -d trpg_platform -v ON_ERROR_STOP=1 -c 'DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = ''trpg_app'') THEN CREATE ROLE trpg_app LOGIN PASSWORD ''trpg_app'' NOSUPERUSER NOBYPASSRLS CREATEDB; ELSE ALTER ROLE trpg_app LOGIN PASSWORD ''trpg_app'' NOSUPERUSER NOBYPASSRLS CREATEDB; END IF; END $$;'

Write-Host "Local DB bootstrap complete."
Write-Host "Next:"
Write-Host "  . .\scripts\dev\db\env.ps1"
Write-Host "  cargo sqlx migrate run --database-url `"$env:TRPG_DATABASE_ADMIN_URL`""
Write-Host "  .\scripts\dev\db\grant-app-role.ps1"
Write-Host "  cargo sqlx prepare --check --workspace"
