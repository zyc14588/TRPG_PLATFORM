#!/bin/sh
set -eu

for role_name in api canonical worker realtime; do
    password_file="/run/secrets/postgres_${role_name}_password"
    password_length="$(tr -d '\r\n' < "$password_file" | wc -c)"
    if [ "$password_length" -lt 24 ]; then
        echo "postgres ${role_name} password secret is missing or too short" >&2
        exit 1
    fi
done

psql --quiet --set=ON_ERROR_STOP=1 \
    --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<'SQL'
SELECT 'CREATE ROLE trpg_api_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION'
 WHERE NOT EXISTS (
     SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_login'
 )
\gexec
ALTER ROLE trpg_api_login
    LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
SELECT format(
    'ALTER ROLE trpg_api_login PASSWORD %L',
    btrim(pg_read_file('/run/secrets/postgres_api_password'))
)
\gexec
GRANT trpg_api_service TO trpg_api_login;

SELECT 'CREATE ROLE trpg_canonical_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION'
 WHERE NOT EXISTS (
     SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_login'
 )
\gexec
ALTER ROLE trpg_canonical_login
    LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
SELECT format(
    'ALTER ROLE trpg_canonical_login PASSWORD %L',
    btrim(pg_read_file('/run/secrets/postgres_canonical_password'))
)
\gexec
GRANT trpg_canonical_service TO trpg_canonical_login;

SELECT 'CREATE ROLE trpg_worker_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION'
 WHERE NOT EXISTS (
     SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_login'
 )
\gexec
ALTER ROLE trpg_worker_login
    LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
SELECT format(
    'ALTER ROLE trpg_worker_login PASSWORD %L',
    btrim(pg_read_file('/run/secrets/postgres_worker_password'))
)
\gexec
GRANT trpg_worker_service TO trpg_worker_login;

SELECT 'CREATE ROLE trpg_realtime_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION'
 WHERE NOT EXISTS (
     SELECT 1 FROM pg_roles WHERE rolname = 'trpg_realtime_login'
 )
\gexec
ALTER ROLE trpg_realtime_login
    LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
SELECT format(
    'ALTER ROLE trpg_realtime_login PASSWORD %L',
    btrim(pg_read_file('/run/secrets/postgres_realtime_password'))
)
\gexec
GRANT trpg_realtime_service TO trpg_realtime_login;
SQL

unset password_file password_length role_name
