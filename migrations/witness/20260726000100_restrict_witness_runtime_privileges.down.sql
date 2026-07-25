REVOKE ALL PRIVILEGES ON TABLE external_audit_witness
    FROM trpg_witness_append_service, trpg_witness_read_service,
         trpg_witness_append_login, trpg_witness_read_login;
REVOKE ALL PRIVILEGES ON SCHEMA public
    FROM trpg_witness_append_service, trpg_witness_read_service,
         trpg_witness_append_login, trpg_witness_read_login;

DO $$
BEGIN
    EXECUTE format(
        'REVOKE ALL PRIVILEGES ON DATABASE %I FROM trpg_witness_append_service, trpg_witness_read_service, trpg_witness_append_login, trpg_witness_read_login',
        current_database()
    );
END;
$$;
