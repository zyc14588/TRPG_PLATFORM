# Responsibility-focused phase sourced by production-security-smoke.sh.
openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 1 \
  -subj "/CN=TRPG Production Security Smoke Root" \
  -keyout "$runtime_directory/ca.key" \
  -out "$runtime_directory/ca.crt" >/dev/null 2>&1
chmod 0600 "$runtime_directory/ca.key"
chmod 0644 "$runtime_directory/ca.crt"
openssl req -x509 -newkey rsa:2048 -nodes -sha256 -days 1 \
  -subj "/CN=AR02 Deliberately Untrusted Root" \
  -keyout "$runtime_directory/wrong-ca.key" \
  -out "$runtime_directory/wrong-ca.crt" >/dev/null 2>&1
chmod 0600 "$runtime_directory/wrong-ca.key"
chmod 0644 "$runtime_directory/wrong-ca.crt"

issue_certificate \
  postgres_server postgres serverAuth \
  "DNS:postgres,DNS:postgres-witness,DNS:localhost,IP:127.0.0.1"
issue_certificate redis_server redis serverAuth \
  "DNS:redis,DNS:localhost,IP:127.0.0.1"
issue_certificate nats_server nats serverAuth \
  "DNS:nats,DNS:localhost,IP:127.0.0.1"
issue_certificate minio_server_v1 minio serverAuth \
  "DNS:minio,DNS:localhost,IP:127.0.0.1"
issue_certificate reverse_proxy reverse-proxy serverAuth \
  "DNS:reverse-proxy,DNS:localhost,IP:127.0.0.1"
issue_certificate redis_client redis-client clientAuth "DNS:redis-client"
issue_certificate redis_healthcheck redis-healthcheck clientAuth "DNS:redis-healthcheck"
issue_certificate nats_client nats-client clientAuth "DNS:nats-client"

postgres_owner_password="$(openssl rand -hex 24)"
postgres_witness_owner_password="$(openssl rand -hex 24)"
postgres_witness_append_password="$(openssl rand -hex 24)"
postgres_witness_read_password="$(openssl rand -hex 24)"
postgres_api_password="$(openssl rand -hex 24)"
postgres_canonical_password="$(openssl rand -hex 24)"
postgres_worker_password="$(openssl rand -hex 24)"
postgres_realtime_password="$(openssl rand -hex 24)"
postgres_backup_password="$(openssl rand -hex 24)"
postgres_restore_password="$(openssl rand -hex 24)"
redis_healthcheck_password="$(openssl rand -hex 24)"
redis_application_password="$(openssl rand -hex 24)"
nats_password="$(openssl rand -hex 24)"
minio_root_user="trpg_root_smoke"
minio_root_password="$(openssl rand -hex 24)"
minio_service_user="trpg_s3_erasure"
minio_service_password="$(openssl rand -hex 24)"

write_secret postgres_bootstrap_password "$postgres_owner_password"
write_secret postgres_witness_owner_password "$postgres_witness_owner_password"
write_secret postgres_witness_append_password "$postgres_witness_append_password"
write_secret postgres_witness_read_password "$postgres_witness_read_password"
write_secret postgres_api_password "$postgres_api_password"
write_secret postgres_canonical_password "$postgres_canonical_password"
write_secret postgres_worker_password "$postgres_worker_password"
write_secret postgres_realtime_password "$postgres_realtime_password"
write_secret postgres_backup_password "$postgres_backup_password"
write_secret postgres_restore_password "$postgres_restore_password"
write_secret owner_database_url \
  "postgresql://trpg_database_owner:$postgres_owner_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret api_database_url \
  "postgresql://trpg_api_login:$postgres_api_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret canonical_database_url \
  "postgresql://trpg_canonical_login:$postgres_canonical_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret worker_database_url \
  "postgresql://trpg_worker_login:$postgres_worker_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret realtime_database_url \
  "postgresql://trpg_realtime_login:$postgres_realtime_password@postgres:5432/coc_ai_trpg?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret witness_owner_database_url \
  "postgresql://trpg_witness_owner:$postgres_witness_owner_password@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret witness_append_database_url \
  "postgresql://trpg_witness_append_login:$postgres_witness_append_password@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret witness_read_database_url \
  "postgresql://trpg_witness_read_login:$postgres_witness_read_password@postgres-witness:5432/coc_ai_trpg_witness?sslmode=verify-full&sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret identity_signing_key "$(openssl rand -hex 32)"
write_secret canonical_hmac_key "$(openssl rand -hex 32)"
write_secret payload_encryption_key "$(openssl rand -hex 32)"
write_secret audit_hmac_key "$(openssl rand -hex 32)"
write_secret local_model_certification_hmac_key "$(openssl rand -hex 32)"
write_secret admin_bootstrap_token "$(openssl rand -hex 32)"
write_secret provider_credential "$(openssl rand -hex 32)"
copy_secret provider_ca_certificate "$runtime_directory/ca.crt"
write_secret admin_pg_service_file \
  "[trpg_backup_source]
host=postgres
port=5432
dbname=coc_ai_trpg
user=trpg_backup_login
sslmode=verify-full
sslrootcert=/run/secrets/postgres_ca_certificate
[trpg_restore_target]
host=postgres
port=5432
dbname=coc_ai_trpg_restore
user=trpg_restore_login
sslmode=verify-full
sslrootcert=/run/secrets/postgres_ca_certificate"
write_secret admin_pg_passfile \
  "postgres:5432:coc_ai_trpg:trpg_backup_login:$postgres_backup_password
postgres:5432:coc_ai_trpg_restore:trpg_restore_login:$postgres_restore_password"
write_secret redis_url "rediss://trpg_runtime:$redis_application_password@redis:6379"
write_secret nats_url "tls://runtime_smoke:$nats_password@nats:4222"
write_secret realtime_cache_key "$(openssl rand -hex 32)"
write_secret object_storage_access_key "$minio_service_user"
write_secret object_storage_secret_key "$minio_service_password"
write_secret redis_healthcheck_password "$redis_healthcheck_password"
write_secret minio_root_user "$minio_root_user"
write_secret minio_root_password "$minio_root_password"
write_secret redis_acl \
  "user default off
user healthcheck on >$redis_healthcheck_password ~* +ping
user trpg_runtime on >$redis_application_password ~* +@all"
write_secret nats_authorization \
  "authorization {
  users = [
    { user: \"runtime_smoke\", password: \"$nats_password\" }
  ]
}"

copy_secret postgres_tls_certificate "$runtime_directory/postgres_server.crt"
copy_secret postgres_tls_private_key "$runtime_directory/postgres_server.key"
copy_secret postgres_ca_certificate "$runtime_directory/ca.crt"
copy_secret redis_tls_certificate "$runtime_directory/redis_server.crt"
copy_secret redis_tls_private_key "$runtime_directory/redis_server.key"
copy_secret redis_tls_ca_certificate "$runtime_directory/ca.crt"
copy_secret redis_client_tls_certificate "$runtime_directory/redis_client.crt"
copy_secret redis_client_tls_private_key "$runtime_directory/redis_client.key"
copy_secret redis_healthcheck_tls_certificate "$runtime_directory/redis_healthcheck.crt"
copy_secret redis_healthcheck_tls_private_key "$runtime_directory/redis_healthcheck.key"
copy_secret nats_tls_certificate "$runtime_directory/nats_server.crt"
copy_secret nats_tls_private_key "$runtime_directory/nats_server.key"
copy_secret nats_tls_ca_certificate "$runtime_directory/ca.crt"
copy_secret nats_client_tls_certificate "$runtime_directory/nats_client.crt"
copy_secret nats_client_tls_private_key "$runtime_directory/nats_client.key"
copy_secret minio_tls_certificate "$runtime_directory/minio_server_v1.crt"
copy_secret minio_tls_private_key "$runtime_directory/minio_server_v1.key"
copy_secret minio_tls_ca_certificate "$runtime_directory/ca.crt"
copy_secret reverse_proxy_tls_certificate "$runtime_directory/reverse_proxy.crt"
copy_secret reverse_proxy_tls_private_key "$runtime_directory/reverse_proxy.key"

# Compose file-backed secrets retain their source mode and ignore per-secret
# uid/gid/mode overrides. The parent remains 0700 on the isolated runner, while
# 0444 mirrors Docker-managed secret mounts and lets non-root service UIDs read
# only the files explicitly mounted into their containers.
chmod 0444 "$secret_directory"/*

export TRPG_COMPOSE_SECRET_DIRECTORY="$secret_directory"
export TRPG_CANONICAL_HMAC_KEY_ID="runtime-smoke-canonical-v1"
export TRPG_PAYLOAD_ENCRYPTION_KEY_ID="runtime-smoke-payload-v1"
export TRPG_AUDIT_HMAC_KEY_ID="runtime-smoke-audit-v1"
export TRPG_OBJECT_STORAGE_BUCKET="trpg-runtime-smoke"
export TRPG_OBJECT_STORAGE_REGION="us-east-1"
export TRPG_IMAGE_TAG="runtime-smoke"
export TRPG_MODEL_PROVIDER_TYPE="cloud"
export TRPG_MODEL_PROVIDER_ID="runtime-smoke-provider"
export TRPG_MODEL_ID="runtime-smoke-model"
export TRPG_MODEL_ARTIFACT_SHA256="sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
export TRPG_MODEL_PROVIDER_BASE_URL="https://provider.invalid/v1"
export TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID="runtime-smoke-provider-route-v1"

python3 "$root/scripts/ci/verify_compose_security.py" --check
"${compose_command[@]}" config --quiet
"${compose_command[@]}" pull postgres postgres-witness redis nats minio minio-init
"${compose_command[@]}" up --detach --wait --wait-timeout 240 \
  postgres postgres-witness redis nats minio
"${compose_command[@]}" run --rm witness-role-bootstrap
"${compose_command[@]}" run --rm minio-init
