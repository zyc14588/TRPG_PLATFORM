\set ON_ERROR_STOP on

BEGIN;

DO $$
DECLARE
    role_name text;
BEGIN
    FOREACH role_name IN ARRAY ARRAY[
        'trpg_application',
        'trpg_api_service',
        'trpg_canonical_service',
        'trpg_worker_service',
        'trpg_realtime_service'
    ]
    LOOP
        IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = role_name) THEN
            EXECUTE format(
                'CREATE ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS',
                role_name
            );
        END IF;
        EXECUTE format(
            'ALTER ROLE %I NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS',
            role_name
        );
    END LOOP;

    FOREACH role_name IN ARRAY ARRAY[
        'trpg_api_login',
        'trpg_canonical_login',
        'trpg_worker_login',
        'trpg_realtime_login'
    ]
    LOOP
        IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = role_name) THEN
            EXECUTE format(
                'CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE INHERIT NOREPLICATION NOBYPASSRLS',
                role_name
            );
        END IF;
        EXECUTE format(
            'ALTER ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE INHERIT NOREPLICATION NOBYPASSRLS',
            role_name
        );
    END LOOP;
END;
$$;

DO $$
DECLARE
    membership_record record;
    managed_roles CONSTANT text[] := ARRAY[
        'trpg_application',
        'trpg_api_service',
        'trpg_canonical_service',
        'trpg_worker_service',
        'trpg_realtime_service',
        'trpg_api_login',
        'trpg_canonical_login',
        'trpg_worker_login',
        'trpg_realtime_login'
    ];
BEGIN
    FOR membership_record IN
        SELECT granted_role.rolname AS granted_role,
               member_role.rolname AS member_role
          FROM pg_auth_members AS membership
          JOIN pg_roles AS granted_role ON granted_role.oid = membership.roleid
          JOIN pg_roles AS member_role ON member_role.oid = membership.member
         WHERE granted_role.rolname = ANY (managed_roles)
            OR member_role.rolname = ANY (managed_roles)
    LOOP
        EXECUTE format(
            'REVOKE %I FROM %I',
            membership_record.granted_role,
            membership_record.member_role
        );
    END LOOP;
END;
$$;

GRANT trpg_api_service TO trpg_api_login;
GRANT trpg_canonical_service TO trpg_canonical_login;
GRANT trpg_worker_service TO trpg_worker_login;
GRANT trpg_realtime_service TO trpg_realtime_login;

COMMIT;
