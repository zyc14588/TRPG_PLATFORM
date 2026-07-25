DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_application') THEN
        CREATE ROLE trpg_application
            NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_api_service') THEN
        CREATE ROLE trpg_api_service
            NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_canonical_service') THEN
        CREATE ROLE trpg_canonical_service
            NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_worker_service') THEN
        CREATE ROLE trpg_worker_service
            NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_realtime_service') THEN
        CREATE ROLE trpg_realtime_service
            NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION;
    END IF;
END;
$$;

-- PostgreSQL grants PUBLIC schema USAGE by default. Leaving that inherited
-- authority in place would make an explicitly revoked role appear isolated
-- while it could still resolve every public relation and function.
REVOKE ALL ON SCHEMA public FROM PUBLIC;
REVOKE ALL ON SCHEMA public FROM trpg_application;
GRANT USAGE ON SCHEMA public
    TO trpg_api_service, trpg_canonical_service,
       trpg_worker_service, trpg_realtime_service;
ALTER DEFAULT PRIVILEGES FOR ROLE trpg_database_owner IN SCHEMA public
    REVOKE ALL ON TABLES FROM trpg_application;
ALTER DEFAULT PRIVILEGES FOR ROLE trpg_database_owner IN SCHEMA public
    REVOKE ALL ON SEQUENCES FROM trpg_application;

-- Table/column privileges are granted by the final schema migration after all
-- objects exist. The legacy aggregate role deliberately has no schema or
-- table privileges; a single application credential must never span API,
-- canonical writer, worker, and realtime trust boundaries.
