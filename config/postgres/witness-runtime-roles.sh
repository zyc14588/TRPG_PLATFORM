#!/bin/sh
set -eu

owner_password="$(tr -d '\r\n' < /run/secrets/postgres_witness_owner_password)"
if [ "${#owner_password}" -lt 24 ]; then
    echo "postgres witness owner password secret is missing or too short" >&2
    exit 1
fi

PGPASSWORD="$owner_password" psql --quiet --set=ON_ERROR_STOP=1 <<'SQL'
DO $$
BEGIN
    IF length(btrim(pg_read_file('/run/secrets/postgres_witness_append_password'))) < 24 THEN
        RAISE EXCEPTION 'postgres witness append password secret is missing or too short';
    END IF;
    IF length(btrim(pg_read_file('/run/secrets/postgres_witness_read_password'))) < 24 THEN
        RAISE EXCEPTION 'postgres witness read password secret is missing or too short';
    END IF;
END;
$$;

SELECT 'CREATE ROLE trpg_witness_append_service NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS'
 WHERE NOT EXISTS (
     SELECT 1 FROM pg_roles WHERE rolname = 'trpg_witness_append_service'
 )
\gexec
ALTER ROLE trpg_witness_append_service
    NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE trpg_witness_append_service RESET ALL;
ALTER ROLE trpg_witness_append_service IN DATABASE coc_ai_trpg_witness RESET ALL;

SELECT 'CREATE ROLE trpg_witness_read_service NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS'
 WHERE NOT EXISTS (
     SELECT 1 FROM pg_roles WHERE rolname = 'trpg_witness_read_service'
 )
\gexec
ALTER ROLE trpg_witness_read_service
    NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE trpg_witness_read_service RESET ALL;
ALTER ROLE trpg_witness_read_service IN DATABASE coc_ai_trpg_witness RESET ALL;

SELECT 'CREATE ROLE trpg_witness_append_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS'
 WHERE NOT EXISTS (
     SELECT 1 FROM pg_roles WHERE rolname = 'trpg_witness_append_login'
 )
\gexec
ALTER ROLE trpg_witness_append_login
    LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE trpg_witness_append_login RESET ALL;
ALTER ROLE trpg_witness_append_login IN DATABASE coc_ai_trpg_witness RESET ALL;
SELECT format(
    'ALTER ROLE trpg_witness_append_login PASSWORD %L',
    btrim(pg_read_file('/run/secrets/postgres_witness_append_password'))
)
\gexec

SELECT 'CREATE ROLE trpg_witness_read_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS'
 WHERE NOT EXISTS (
     SELECT 1 FROM pg_roles WHERE rolname = 'trpg_witness_read_login'
 )
\gexec
ALTER ROLE trpg_witness_read_login
    LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE trpg_witness_read_login RESET ALL;
ALTER ROLE trpg_witness_read_login IN DATABASE coc_ai_trpg_witness RESET ALL;
SELECT format(
    'ALTER ROLE trpg_witness_read_login PASSWORD %L',
    btrim(pg_read_file('/run/secrets/postgres_witness_read_password'))
)
\gexec

-- Normalize role membership instead of trusting a pre-existing data volume.
-- This removes stale or malicious grants (including predefined/admin roles)
-- before installing the two intended service memberships.
SELECT format('REVOKE %I FROM %I', granted.rolname, member.rolname)
  FROM pg_auth_members membership
  JOIN pg_roles granted ON granted.oid = membership.roleid
  JOIN pg_roles member ON member.oid = membership.member
 WHERE member.rolname IN (
     'trpg_witness_append_service',
     'trpg_witness_read_service',
     'trpg_witness_append_login',
     'trpg_witness_read_login'
 )
\gexec

GRANT trpg_witness_append_service TO trpg_witness_append_login;
GRANT trpg_witness_read_service TO trpg_witness_read_login;

-- Re-apply the privilege boundary on every startup. This repairs stale grants
-- on an existing volume even when the SQLx migration was already recorded.
DO $$
BEGIN
    EXECUTE format(
        'REVOKE ALL PRIVILEGES ON DATABASE %I FROM PUBLIC',
        current_database()
    );
    EXECUTE format(
        'REVOKE ALL PRIVILEGES ON DATABASE %I FROM trpg_witness_append_service, trpg_witness_read_service, trpg_witness_append_login, trpg_witness_read_login',
        current_database()
    );
    EXECUTE format(
        'GRANT CONNECT ON DATABASE %I TO trpg_witness_append_service, trpg_witness_read_service',
        current_database()
    );
END;
$$;

REVOKE CREATE ON SCHEMA public FROM PUBLIC;
REVOKE ALL PRIVILEGES ON SCHEMA public
    FROM trpg_witness_append_service, trpg_witness_read_service,
         trpg_witness_append_login, trpg_witness_read_login;
REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM PUBLIC;
REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public
    FROM trpg_witness_append_service, trpg_witness_read_service,
         trpg_witness_append_login, trpg_witness_read_login;
REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public FROM PUBLIC;
REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public
    FROM trpg_witness_append_service, trpg_witness_read_service,
         trpg_witness_append_login, trpg_witness_read_login;
GRANT USAGE ON SCHEMA public
    TO trpg_witness_append_service, trpg_witness_read_service;

DO $$
BEGIN
    IF to_regclass('public.external_audit_witness') IS NOT NULL THEN
        GRANT SELECT, INSERT ON TABLE external_audit_witness
            TO trpg_witness_append_service;
        GRANT SELECT ON TABLE external_audit_witness
            TO trpg_witness_read_service;
    END IF;
END;
$$;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    REVOKE ALL PRIVILEGES ON TABLES FROM PUBLIC;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    REVOKE ALL PRIVILEGES ON SEQUENCES FROM PUBLIC;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
          FROM pg_shdepend dependency
          JOIN pg_roles owner_role ON owner_role.oid = dependency.refobjid
         WHERE dependency.refclassid = 'pg_authid'::regclass
           AND dependency.deptype = 'o'
           AND owner_role.rolname IN (
               'trpg_witness_append_service',
               'trpg_witness_read_service',
               'trpg_witness_append_login',
               'trpg_witness_read_login'
           )
    ) THEN
        RAISE EXCEPTION 'witness runtime role unexpectedly owns a database object';
    END IF;
END;
$$;
SQL

unset owner_password
