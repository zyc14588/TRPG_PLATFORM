$ErrorActionPreference = "Stop"

$container = "trpg-platform-pgvector"

function Invoke-DockerPsql {
  param([string]$Sql)

  docker exec $container psql -U postgres -d trpg_platform -v ON_ERROR_STOP=1 -c $Sql
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

function Invoke-DockerTemplatePsql {
  param([string]$Sql)

  docker exec $container psql -U postgres -d template1 -v ON_ERROR_STOP=1 -c $Sql
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

Write-Host "Preparing template1 extensions for disposable storage test databases..."
Invoke-DockerTemplatePsql "CREATE EXTENSION IF NOT EXISTS pgcrypto;"
Invoke-DockerTemplatePsql "CREATE EXTENSION IF NOT EXISTS vector;"

Write-Host "Granting local schema/table access to trpg_app for SQLx prepare and app-role RLS tests..."
Invoke-DockerPsql 'DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = ''trpg_app'') THEN CREATE ROLE trpg_app LOGIN PASSWORD ''trpg_app'' NOSUPERUSER NOBYPASSRLS CREATEDB; ELSE ALTER ROLE trpg_app LOGIN PASSWORD ''trpg_app'' NOSUPERUSER NOBYPASSRLS CREATEDB; END IF; END $$;'
Invoke-DockerPsql "GRANT CONNECT ON DATABASE trpg_platform TO trpg_app;"
Invoke-DockerPsql "GRANT USAGE ON SCHEMA public TO trpg_app;"
Invoke-DockerPsql "GRANT USAGE ON SCHEMA app TO trpg_app;"
Invoke-DockerPsql "GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA app TO trpg_app;"
Invoke-DockerPsql "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO trpg_app;"
Invoke-DockerPsql "GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO trpg_app;"
Invoke-DockerPsql "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO trpg_app;"
Invoke-DockerPsql "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO trpg_app;"
Invoke-DockerPsql "ALTER DEFAULT PRIVILEGES IN SCHEMA app GRANT EXECUTE ON FUNCTIONS TO trpg_app;"
Invoke-DockerPsql "GRANT trpg_app_private TO trpg_app;"

Write-Host "Preparing local non-login RLS proof role..."
Invoke-DockerPsql 'DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = ''trpg_rls_test'') THEN CREATE ROLE trpg_rls_test; END IF; END $$;'
Invoke-DockerPsql "ALTER ROLE trpg_rls_test NOSUPERUSER NOBYPASSRLS NOLOGIN;"
Invoke-DockerPsql "GRANT USAGE ON SCHEMA public, app TO trpg_rls_test;"
Invoke-DockerPsql "GRANT SELECT ON ALL TABLES IN SCHEMA public TO trpg_rls_test;"
Invoke-DockerPsql "GRANT INSERT, UPDATE, DELETE ON document_sources, documents, chunks, ingest_jobs TO trpg_rls_test;"
Invoke-DockerPsql "GRANT trpg_rls_test TO trpg_app;"

Write-Host "Grant complete."
