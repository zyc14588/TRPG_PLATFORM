# Responsibility-focused phase sourced by production-security-smoke.sh.
# All Rust services intentionally share one runtime image. Building every
# service in parallel asks Buildx to export the same tag five times and can
# race its snapshot extraction. Build each distinct target once, then require
# the full graph to start from exactly those local images.
"${compose_command[@]}" build api web
"${compose_command[@]}" up --detach --no-build --wait --wait-timeout 600

# Seed an existing-volume privilege regression, then require the idempotent
# bootstrap to remove every direct grant, dangerous role flag, role setting,
# and owner membership. The exact matrices below prove the repair took effect.
witness_query trpg_witness_owner "$postgres_witness_owner_password" "
GRANT CREATE, TEMPORARY ON DATABASE coc_ai_trpg_witness
    TO trpg_witness_read_login;
GRANT CREATE ON SCHEMA public TO trpg_witness_read_login;
GRANT UPDATE, DELETE, TRUNCATE ON TABLE external_audit_witness
    TO trpg_witness_append_login;
GRANT INSERT ON TABLE external_audit_witness TO trpg_witness_read_login;
GRANT trpg_witness_owner TO trpg_witness_append_login;
ALTER ROLE trpg_witness_append_login CREATEDB CREATEROLE BYPASSRLS;
ALTER ROLE trpg_witness_append_login SET search_path = pg_catalog;
" >/dev/null
"${compose_command[@]}" run --rm witness-role-bootstrap

append_privileges="$(
  witness_query trpg_witness_append_login "$postgres_witness_append_password" "
SELECT concat_ws(
    '|',
    current_user,
    has_database_privilege(current_user, current_database(), 'CONNECT'),
    has_database_privilege(current_user, current_database(), 'CREATE'),
    has_database_privilege(current_user, current_database(), 'TEMPORARY'),
    has_schema_privilege(current_user, 'public', 'USAGE'),
    has_schema_privilege(current_user, 'public', 'CREATE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'SELECT'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'INSERT'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'UPDATE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'DELETE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'TRUNCATE'),
    pg_has_role(current_user, 'trpg_witness_owner', 'MEMBER'),
    pg_has_role(current_user, 'trpg_witness_append_service', 'MEMBER'),
    pg_has_role(current_user, 'trpg_witness_read_service', 'MEMBER'),
    rolsuper,
    rolcreatedb,
    rolcreaterole,
    rolreplication,
    rolbypassrls
)
FROM pg_roles
WHERE rolname = current_user;
"
)"
expected_append_privileges="trpg_witness_append_login|t|f|f|t|f|t|t|f|f|f|f|t|f|f|f|f|f|f"
if [[ "$append_privileges" != "$expected_append_privileges" ]]; then
  printf 'PostgreSQL witness append privilege matrix is not least-privilege: %s\n' \
    "$append_privileges" >&2
  exit 1
fi

read_privileges="$(
  witness_query trpg_witness_read_login "$postgres_witness_read_password" "
SELECT concat_ws(
    '|',
    current_user,
    has_database_privilege(current_user, current_database(), 'CONNECT'),
    has_database_privilege(current_user, current_database(), 'CREATE'),
    has_database_privilege(current_user, current_database(), 'TEMPORARY'),
    has_schema_privilege(current_user, 'public', 'USAGE'),
    has_schema_privilege(current_user, 'public', 'CREATE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'SELECT'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'INSERT'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'UPDATE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'DELETE'),
    has_table_privilege(current_user, 'public.external_audit_witness', 'TRUNCATE'),
    pg_has_role(current_user, 'trpg_witness_owner', 'MEMBER'),
    pg_has_role(current_user, 'trpg_witness_append_service', 'MEMBER'),
    pg_has_role(current_user, 'trpg_witness_read_service', 'MEMBER'),
    rolsuper,
    rolcreatedb,
    rolcreaterole,
    rolreplication,
    rolbypassrls
)
FROM pg_roles
WHERE rolname = current_user;
"
)"
expected_read_privileges="trpg_witness_read_login|t|f|f|t|f|t|f|f|f|f|f|f|t|f|f|f|f|f"
if [[ "$read_privileges" != "$expected_read_privileges" ]]; then
  printf 'PostgreSQL witness read privilege matrix is not least-privilege: %s\n' \
    "$read_privileges" >&2
  exit 1
fi

witness_query trpg_witness_read_login "$postgres_witness_read_password" \
  "SELECT count(*) FROM external_audit_witness;" >/dev/null

witness_query trpg_witness_append_login "$postgres_witness_append_password" "
BEGIN;
INSERT INTO external_audit_witness (
    sequence,
    commit_id,
    phase,
    primary_request_hash,
    primary_first_sequence,
    primary_last_sequence,
    reason,
    integrity_key_id,
    previous_hash,
    record_hash
)
SELECT
    0,
    'runtime-privilege-probe-' || txid_current()::text,
    'PREPARED',
    'sha256:' || lpad(to_hex(txid_current()), 64, '0'),
    NULL,
    NULL,
    'least-privilege smoke probe',
    'runtime-privilege-probe',
    COALESCE(
        (
            SELECT record_hash
            FROM external_audit_witness
            ORDER BY sequence DESC
            LIMIT 1
        ),
        'hmac-sha256:' || repeat('0', 64)
    ),
    'hmac-sha256:' || lpad(to_hex(txid_current()), 64, '0');
ROLLBACK;
" >/dev/null

expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" update \
  "BEGIN; UPDATE external_audit_witness SET reason = reason WHERE false; ROLLBACK;"
expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" delete \
  "BEGIN; DELETE FROM external_audit_witness WHERE false; ROLLBACK;"
expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" truncate \
  "BEGIN; TRUNCATE TABLE external_audit_witness; ROLLBACK;"
expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" drop-table \
  "BEGIN; DROP TABLE external_audit_witness; ROLLBACK;"
expect_witness_denied \
  trpg_witness_append_login "$postgres_witness_append_password" create-table \
  "BEGIN; CREATE TABLE witness_privilege_escape_probe(id integer); ROLLBACK;"
expect_witness_denied \
  trpg_witness_read_login "$postgres_witness_read_password" insert \
  "
BEGIN;
INSERT INTO external_audit_witness (
    sequence,
    commit_id,
    phase,
    primary_request_hash,
    primary_first_sequence,
    primary_last_sequence,
    reason,
    integrity_key_id,
    previous_hash,
    record_hash
)
SELECT
    0,
    'read-privilege-escape-' || txid_current()::text,
    'PREPARED',
    'sha256:' || lpad(to_hex(txid_current()), 64, '0'),
    NULL,
    NULL,
    'read privilege escape probe',
    'runtime-privilege-probe',
    COALESCE(
        (
            SELECT record_hash
            FROM external_audit_witness
            ORDER BY sequence DESC
            LIMIT 1
        ),
        'hmac-sha256:' || repeat('0', 64)
    ),
    'hmac-sha256:' || lpad(to_hex(txid_current()), 64, '0');
ROLLBACK;
"
