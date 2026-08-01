
server_major="$(
  docker exec trpg-primary-postgres postgres --version |
    sed -E 's/.* ([0-9]+)([.].*)?$/\1/'
)"
postgres_bindir=""
if [[ -n "${TRPG_POSTGRES_BINDIR:-}" ]]; then
  postgres_bindir="$TRPG_POSTGRES_BINDIR"
elif command -v pg_config >/dev/null 2>&1; then
  postgres_bindir="$(pg_config --bindir)"
fi

use_container_client=true
if [[ -n "$postgres_bindir" ]]; then
  host_pg_dump="$postgres_bindir/pg_dump"
  host_pg_restore="$postgres_bindir/pg_restore"
  host_psql="$postgres_bindir/psql"
  if [[ -f "$host_pg_dump" && ! -L "$host_pg_dump" &&
        -f "$host_pg_restore" && ! -L "$host_pg_restore" &&
        -f "$host_psql" && ! -L "$host_psql" ]]; then
    host_dump_major="$(
      "$host_pg_dump" --version |
        sed -E 's/.* ([0-9]+)([.].*)?$/\1/'
    )"
    host_restore_major="$(
      "$host_pg_restore" --version |
        sed -E 's/.* ([0-9]+)([.].*)?$/\1/'
    )"
    host_psql_major="$(
      "$host_psql" --version |
        sed -E 's/.* ([0-9]+)([.].*)?$/\1/'
    )"
    if [[ "$host_dump_major" == "$server_major" &&
          "$host_restore_major" == "$server_major" &&
          "$host_psql_major" == "$server_major" ]]; then
      pg_dump_path="$host_pg_dump"
      pg_restore_path="$host_pg_restore"
      psql_path="$host_psql"
      use_container_client=false
    elif [[ -n "${TRPG_POSTGRES_BINDIR:-}" ]]; then
      printf 'explicit PostgreSQL client majors %s/%s/%s do not match server major %s\n' \
        "$host_dump_major" "$host_restore_major" "$host_psql_major" "$server_major" >&2
      exit 1
    fi
  elif [[ -n "${TRPG_POSTGRES_BINDIR:-}" ]]; then
    printf 'explicit PostgreSQL bindir does not contain regular non-symlink tools\n' >&2
    exit 1
  fi
fi

export TRPG_POSTGRES_CLIENT_IMAGE="$postgres_client_image"
export TRPG_POSTGRES_CLIENT_MOUNT_ROOT="$runtime_root"
if [[ "$use_container_client" == true ]]; then
  postgres_wrapper_directory="$backup_directory/postgres-client"
  install -d -m 0700 "$postgres_wrapper_directory"
  install -m 0755 "$root/scripts/ci/postgres-container-client.sh" \
    "$postgres_wrapper_directory/pg_dump"
  install -m 0755 "$root/scripts/ci/postgres-container-client.sh" \
    "$postgres_wrapper_directory/pg_restore"
  install -m 0755 "$root/scripts/ci/postgres-container-client.sh" \
    "$postgres_wrapper_directory/psql"
  pg_dump_path="$postgres_wrapper_directory/pg_dump"
  pg_restore_path="$postgres_wrapper_directory/pg_restore"
  psql_path="$postgres_wrapper_directory/psql"
fi

for postgres_program in "$pg_dump_path" "$pg_restore_path" "$psql_path"; do
  if [[ ! -f "$postgres_program" || -L "$postgres_program" ]]; then
    printf 'PostgreSQL tool must be a regular non-symlink file: %s\n' "$postgres_program" >&2
    exit 1
  fi
done
dump_major="$("$pg_dump_path" --version | sed -E 's/.* ([0-9]+)([.].*)?$/\1/')"
restore_major="$("$pg_restore_path" --version | sed -E 's/.* ([0-9]+)([.].*)?$/\1/')"
psql_major="$("$psql_path" --version | sed -E 's/.* ([0-9]+)([.].*)?$/\1/')"
if [[ "$dump_major" != "$server_major" || "$restore_major" != "$server_major" ||
      "$psql_major" != "$server_major" ]]; then
  printf 'PostgreSQL client majors %s/%s/%s do not match server major %s\n' \
    "$dump_major" "$restore_major" "$psql_major" "$server_major" >&2
  exit 1
fi
schema_assertion_path="$runtime_root/assert-schema.sql"
install -m 0600 "$root/scripts/ci/assert-schema.sql" "$schema_assertion_path"

for migration in "$root"/migrations/*.sql; do
  if [[ "$migration" == *.down.sql ]]; then
    continue
  fi
  docker exec -i trpg-primary-postgres \
    psql -X -v ON_ERROR_STOP=1 -1 -U postgres -d trpg_backup_source \
    <"$migration" >/dev/null
done

{
  printf '[trpg_backup_source]\n'
  printf 'host=127.0.0.1\nport=15432\nuser=postgres\npassword=%s\ndbname=trpg_backup_source\nsslmode=disable\n\n' \
    "$postgres_password"
  printf '[trpg_backup_target]\n'
  printf 'host=127.0.0.1\nport=15432\nuser=postgres\npassword=%s\ndbname=trpg_backup_target\nsslmode=disable\n' \
    "$postgres_password"
} >"$libpq_service_file"
chmod 0600 "$libpq_service_file"

cat >>"$github_env" <<ENVIRONMENT
P02_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_identity
P02_WORKER_SERVICE_DATABASE_URL=postgresql://trpg_worker_login:${postgres_password}@127.0.0.1:15432/p02_identity
P02_CANONICAL_SERVICE_DATABASE_URL=postgresql://trpg_canonical_login:${postgres_password}@127.0.0.1:15432/p02_identity
P02_REDIS_URL=redis://127.0.0.1:16379
AR03_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/ar03_ledger_witness
P02_CANONICAL_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_canonical
P02_CANONICAL_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_canonical_witness
P02_CANONICAL_ALLOW_DATABASE_RESET=1
P02_CANONICAL_RESET_DATABASE=p02_canonical
P02_CANONICAL_WITNESS_RESET_DATABASE=p02_canonical_witness
P02_EVENTING_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_eventing
P02_EVENTING_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_eventing_witness
P02_EVENTING_ALLOW_DATABASE_RESET=1
P02_EVENTING_RESET_DATABASE=p02_eventing
P02_EVENTING_WITNESS_RESET_DATABASE=p02_eventing_witness
P02_WORKFLOW_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_workflow
P02_API_CANONICAL_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_api_replay
P02_API_CANONICAL_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_api_replay_witness
P02_FORMAL_COMMIT_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p02_formal_commit
P02_FORMAL_COMMIT_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_formal_commit_witness
P02_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p02_service_witness
P02_WITNESS_APPEND_DATABASE_URL=postgresql://trpg_witness_append_login:${witness_append_password}@127.0.0.1:15433/p02_service_witness
P02_NATS_URL=nats://127.0.0.1:14222
P03_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p03_migration_upgrade
P03_ALLOW_DATABASE_RESET=1
P03_SCHEMA_ASSERTION_PATH=${schema_assertion_path}
P04_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p04_eventing
P04_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p04_eventing_witness
P04_ALLOW_DATABASE_RESET=1
P04_ADMIN_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/postgres
P04_RECOVERY_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p04_eventing_recovery
P05_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p05_privacy
P05_WORKER_DATABASE_URL=postgresql://trpg_worker_login:${postgres_password}@127.0.0.1:15432/p05_privacy
P05_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p05_privacy_witness
P06_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p06_core_domain
P06_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p06_core_domain_witness
P06_ALLOW_DATABASE_RESET=1
P06_RESET_DATABASE=p06_core_domain
P06_WITNESS_RESET_DATABASE=p06_core_domain_witness
P07_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p07_player_action
P07_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p07_player_action_witness
P07_ALLOW_DATABASE_RESET=1
P07_RESET_DATABASE=p07_player_action
P07_WITNESS_RESET_DATABASE=p07_player_action_witness
P07_NATS_URL=nats://127.0.0.1:14222
P08_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/p08_tutorial
P08_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/p08_tutorial_witness
P08_ALLOW_DATABASE_RESET=1
P08_RESET_DATABASE=p08_tutorial
P08_WITNESS_RESET_DATABASE=p08_tutorial_witness
AR06_ADMIN_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/v1_lifecycle_api
AR06_ADMIN_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/v1_lifecycle_api_witness
AR06_API_DATABASE_URL=postgresql://trpg_api_login:${postgres_password}@127.0.0.1:15432/v1_lifecycle_api
AR06_CANONICAL_DATABASE_URL=postgresql://trpg_canonical_login:${postgres_password}@127.0.0.1:15432/v1_lifecycle_api
AR06_WITNESS_DATABASE_URL=postgresql://trpg_witness_append_login:${witness_append_password}@127.0.0.1:15433/v1_lifecycle_api_witness
AR06_REDIS_URL=redis://127.0.0.1:16379
AR07_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/realtime_transport
AR07_REALTIME_DATABASE_URL=postgresql://trpg_realtime_login:${postgres_password}@127.0.0.1:15432/realtime_transport
AR07_WITNESS_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15433/realtime_transport_witness
AR07_NATS_URL=nats://127.0.0.1:14222
AR07_DATABASE_NAME=realtime_transport
AR07_WITNESS_DATABASE_NAME=realtime_transport_witness
AR07_ALLOW_DATABASE_RESET=1
AR09_AGENT_JOB_FIXTURE_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/ar09_agent_jobs
AR09_AGENT_JOB_API_DATABASE_URL=postgresql://trpg_api_login:${postgres_password}@127.0.0.1:15432/ar09_agent_jobs
AR09_AGENT_JOB_WORKER_DATABASE_URL=postgresql://trpg_worker_login:${postgres_password}@127.0.0.1:15432/ar09_agent_jobs
AR09_AGENT_JOB_CANONICAL_DATABASE_URL=postgresql://trpg_canonical_login:${postgres_password}@127.0.0.1:15432/ar09_agent_jobs
AR09_PUBLIC_FIXTURE_DATABASE_URL=postgresql://postgres:${postgres_password}@127.0.0.1:15432/ar09_agent_jobs_public
AR09_PUBLIC_API_DATABASE_URL=postgresql://trpg_api_login:${postgres_password}@127.0.0.1:15432/ar09_agent_jobs_public
AR09_PUBLIC_WORKER_DATABASE_URL=postgresql://trpg_worker_login:${postgres_password}@127.0.0.1:15432/ar09_agent_jobs_public
AR09_PUBLIC_CANONICAL_DATABASE_URL=postgresql://trpg_canonical_login:${postgres_password}@127.0.0.1:15432/ar09_agent_jobs_public
AR09_PUBLIC_WITNESS_DATABASE_URL=postgresql://trpg_witness_append_login:${witness_append_password}@127.0.0.1:15433/ar09_agent_jobs_public_witness
AR09_PUBLIC_REDIS_URL=redis://127.0.0.1:16379
TRPG_POSTGRES_CLIENT_IMAGE=${postgres_client_image}
TRPG_POSTGRES_CLIENT_MOUNT_ROOT=${runtime_root}
TMPDIR=${runtime_root}
P05_REDIS_URL=redis://127.0.0.1:16379
P05_NATS_URL=nats://127.0.0.1:14222
P05_MINIO_ENDPOINT=https://localhost:19000
P05_MINIO_REGION=us-east-1
P05_MINIO_BUCKET=trpg-ci-deletion
P05_MINIO_CA_CERT_PATH=${minio_ca_bundle}
SSL_CERT_FILE=${minio_ca_bundle}
ENVIRONMENT

{
  printf 'P02_TLS_DATABASE_URL=postgresql://postgres:%s@%s:15434/p02_tls_identity?sslmode=require\n' \
    "$postgres_password" "$tls_hostname"
  printf 'P02_TLS_CA_CERT_PATH=%s\n' "$tls_directory/ca.crt"
  printf 'P02_PG_DUMP=%s\n' "$pg_dump_path"
  printf 'P02_PG_RESTORE=%s\n' "$pg_restore_path"
  printf 'P02_PSQL=%s\n' "$psql_path"
  printf 'P02_LIBPQ_SERVICE_FILE=%s\n' "$libpq_service_file"
  printf 'P02_BACKUP_SOURCE_SERVICE=trpg_backup_source\n'
  printf 'P02_BACKUP_TARGET_SERVICE=trpg_backup_target\n'
  printf 'P02_BACKUP_SOURCE_URL=postgresql://postgres:%s@127.0.0.1:15432/trpg_backup_source\n' \
    "$postgres_password"
  printf 'P02_BACKUP_TARGET_URL=postgresql://postgres:%s@127.0.0.1:15432/trpg_backup_target\n' \
    "$postgres_password"
  printf 'P02_BACKUP_DIR=%s\n' "$backup_directory/artifacts"
  printf 'P04_PG_DUMP=%s\n' "$pg_dump_path"
  printf 'P04_PG_RESTORE=%s\n' "$pg_restore_path"
  printf 'P05_MINIO_ACCESS_KEY=%s\n' "$minio_service_access_key"
  printf 'P05_MINIO_SECRET_KEY=%s\n' "$minio_service_secret_key"
} >>"$github_env"

python3 "$root/scripts/ci/p02_policy_bootstrap.py" --github-env "$github_env"

policy_environment_value() {
  local name="$1"
  awk -F= -v name="$name" '$1 == name { value = substr($0, length(name) + 2) } END { print value }' \
    "$github_env"
}
ar06_openfga_address="$(policy_environment_value P02_OPENFGA_ADDRESS)"
ar06_openfga_store_id="$(policy_environment_value P02_OPENFGA_STORE_ID)"
ar06_openfga_model_id="$(policy_environment_value P02_OPENFGA_MODEL_ID)"
ar06_opa_address="$(policy_environment_value P02_OPA_ADDRESS)"
ar06_opa_revision="$(policy_environment_value P02_OPA_REVISION)"
for policy_value in "$ar06_openfga_address" "$ar06_openfga_store_id" \
  "$ar06_openfga_model_id" "$ar06_opa_address" "$ar06_opa_revision"; do
  [[ -n "$policy_value" ]] || { printf 'policy bootstrap environment is incomplete\n' >&2; exit 1; }
done
cat >>"$github_env" <<ENVIRONMENT
AR06_OPENFGA_ADDRESS=${ar06_openfga_address}
AR06_OPENFGA_STORE_ID=${ar06_openfga_store_id}
AR06_OPENFGA_MODEL_ID=${ar06_openfga_model_id}
AR06_OPA_ADDRESS=${ar06_opa_address}
AR06_OPA_REVISION=${ar06_opa_revision}
ENVIRONMENT
