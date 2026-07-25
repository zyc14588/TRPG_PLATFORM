-- Runtime services must never receive the independently operated witness
-- database owner credential. The API may append and read witness records;
-- realtime and worker processes may only verify the chain. Schema migration
-- and ownership remain exclusive to the witness owner.

DO $$
BEGIN
    BEGIN
        CREATE ROLE trpg_witness_append_service
            NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE
            NOREPLICATION NOBYPASSRLS;
    EXCEPTION
        WHEN duplicate_object THEN NULL;
    END;
    BEGIN
        CREATE ROLE trpg_witness_read_service
            NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE
            NOREPLICATION NOBYPASSRLS;
    EXCEPTION
        WHEN duplicate_object THEN NULL;
    END;
    BEGIN
        CREATE ROLE trpg_witness_append_login
            NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE
            NOREPLICATION NOBYPASSRLS;
    EXCEPTION
        WHEN duplicate_object THEN NULL;
    END;
    BEGIN
        CREATE ROLE trpg_witness_read_login
            NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE
            NOREPLICATION NOBYPASSRLS;
    EXCEPTION
        WHEN duplicate_object THEN NULL;
    END;
END;
$$;

ALTER ROLE trpg_witness_append_service
    NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE
    NOREPLICATION NOBYPASSRLS;
ALTER ROLE trpg_witness_read_service
    NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE
    NOREPLICATION NOBYPASSRLS;

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
GRANT SELECT, INSERT ON TABLE external_audit_witness
    TO trpg_witness_append_service;
GRANT SELECT ON TABLE external_audit_witness
    TO trpg_witness_read_service;

ALTER DEFAULT PRIVILEGES IN SCHEMA public
    REVOKE ALL PRIVILEGES ON TABLES FROM PUBLIC;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    REVOKE ALL PRIVILEGES ON SEQUENCES FROM PUBLIC;
