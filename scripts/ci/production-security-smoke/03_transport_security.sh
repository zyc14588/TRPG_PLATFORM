# Responsibility-focused phase sourced by production-security-smoke.sh.
if PGPASSWORD="$postgres_api_password" PGSSLMODE=disable \
  psql -X -h localhost -p 25432 -U trpg_api_login -d coc_ai_trpg \
  -c "SELECT 1" >/dev/null 2>&1; then
  printf 'PostgreSQL accepted a plaintext TCP connection\n' >&2
  exit 1
fi
postgres_tls="$(
  PGPASSWORD="$postgres_api_password" \
  PGSSLMODE=verify-full \
  PGSSLROOTCERT="$runtime_directory/ca.crt" \
  psql -X -A -t -h localhost -p 25432 \
    -U trpg_api_login -d coc_ai_trpg \
    -c "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()"
)"
if [[ "$postgres_tls" != t ]]; then
  printf 'PostgreSQL TLS verification did not reach an SSL session\n' >&2
  exit 1
fi

if PGPASSWORD="$postgres_witness_append_password" PGSSLMODE=disable \
  psql -X -h localhost -p 25433 -U trpg_witness_append_login -d coc_ai_trpg_witness \
  -c "SELECT 1" >/dev/null 2>&1; then
  printf 'PostgreSQL witness accepted a plaintext TCP connection\n' >&2
  exit 1
fi
witness_tls="$(
  PGPASSWORD="$postgres_witness_append_password" \
  PGSSLMODE=verify-full \
  PGSSLROOTCERT="$runtime_directory/ca.crt" \
  psql -X -A -t -h localhost -p 25433 \
    -U trpg_witness_append_login -d coc_ai_trpg_witness \
    -c "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()"
)"
if [[ "$witness_tls" != t ]]; then
  printf 'PostgreSQL witness TLS verification did not reach an SSL session\n' >&2
  exit 1
fi

redis_probe() {
  printf "*3\r\n\$4\r\nAUTH\r\n\$11\r\nhealthcheck\r\n\$%s\r\n%s\r\n*1\r\n\$4\r\nPING\r\n" \
    "${#redis_healthcheck_password}" "$redis_healthcheck_password" |
    timeout 10 openssl s_client "$@"
}

redis_without_client="$(
  redis_probe \
    -connect localhost:26379 \
    -servername localhost \
    -CAfile "$runtime_directory/ca.crt" \
    -verify_return_error \
    -quiet 2>/dev/null || true
)"
if [[ "$redis_without_client" == *PONG* ]]; then
  printf 'Redis accepted TLS without a client certificate\n' >&2
  exit 1
fi
redis_with_client="$(
  redis_probe \
    -connect localhost:26379 \
    -servername localhost \
    -CAfile "$runtime_directory/ca.crt" \
    -cert "$runtime_directory/redis_healthcheck.crt" \
    -key "$runtime_directory/redis_healthcheck.key" \
    -verify_return_error \
    -quiet 2>/dev/null || true
)"
if [[ "$redis_with_client" != *PONG* ]]; then
  printf 'Redis mTLS client did not receive PONG\n' >&2
  exit 1
fi

nats_request="CONNECT {\"user\":\"runtime_smoke\",\"pass\":\"$nats_password\",\"verbose\":false}"$'\r\nPING\r\n'
nats_without_client="$(
  printf '%s' "$nats_request" |
    timeout 10 openssl s_client \
      -connect localhost:24222 \
      -servername localhost \
      -CAfile "$runtime_directory/ca.crt" \
      -verify_return_error \
      -quiet 2>/dev/null || true
)"
if [[ "$nats_without_client" == *PONG* ]]; then
  printf 'NATS accepted TLS without a client certificate\n' >&2
  exit 1
fi
nats_with_client="$(
  printf '%s' "$nats_request" |
    timeout 10 openssl s_client \
      -connect localhost:24222 \
      -servername localhost \
      -CAfile "$runtime_directory/ca.crt" \
      -cert "$runtime_directory/nats_client.crt" \
      -key "$runtime_directory/nats_client.key" \
      -verify_return_error \
      -quiet 2>/dev/null || true
)"
if [[ "$nats_with_client" != *PONG* ]]; then
  printf 'NATS mTLS client did not receive PONG\n' >&2
  exit 1
fi

curl --fail --silent --show-error \
  --cacert "$runtime_directory/ca.crt" \
  https://localhost:29000/minio/health/live >/dev/null

# Re-run the root-only bootstrap to prove that policy and service-identity
# reconciliation is idempotent on an existing volume, then exercise the real
# version-aware Rust deletion adapter against the rotated certificate.
"${compose_command[@]}" run --rm minio-init
SSL_CERT_FILE="$runtime_directory/ca.crt" \
AR02_MINIO_ENDPOINT="https://localhost:29000" \
AR02_MINIO_REGION="$TRPG_OBJECT_STORAGE_REGION" \
AR02_MINIO_BUCKET="$TRPG_OBJECT_STORAGE_BUCKET" \
AR02_MINIO_ROOT_ACCESS_KEY="$minio_root_user" \
AR02_MINIO_ROOT_SECRET_KEY="$minio_root_password" \
AR02_MINIO_SERVICE_ACCESS_KEY="$minio_service_user" \
AR02_MINIO_SERVICE_SECRET_KEY="$minio_service_password" \
AR02_MINIO_CA_CERT_PATH="$runtime_directory/ca.crt" \
  cargo test -p trpg-security-governance \
    ar02_live_s3_version_erasure_closes_recoverable_history \
    -- --ignored --nocapture

# Exercise the exact application identity from inside the isolated Compose
# network. Root is used only to create and remove the negative-test bucket.
"${compose_command[@]}" run --rm --entrypoint sh minio-init -ec '
  set -eu
  mkdir -p /tmp/mc/certs/CAs
  cp /run/secrets/minio_tls_ca_certificate /tmp/mc/certs/CAs/trpg-ca.crt
  chmod 0644 /tmp/mc/certs/CAs/trpg-ca.crt
  export MC_CERTS_DIR=/tmp/mc/certs
  bucket="${TRPG_OBJECT_STORAGE_BUCKET:-trpg-private-data}"
  forbidden_bucket="${bucket}-forbidden"
  root_access="$(cat /run/secrets/minio_root_user)"
  root_secret="$(cat /run/secrets/minio_root_password)"
  service_access="$(cat /run/secrets/object_storage_access_key)"
  service_secret="$(cat /run/secrets/object_storage_secret_key)"
  export MC_HOST_root="https://${root_access}:${root_secret}@minio:9000"
  export MC_HOST_service="https://${service_access}:${service_secret}@minio:9000"
  mc --config-dir /tmp/mc mb --ignore-existing "root/${forbidden_bucket}"
  if mc --config-dir /tmp/mc ls service >/dev/null 2>&1; then
    printf "object-storage service identity listed all buckets\n" >&2
    exit 1
  fi
  if mc --config-dir /tmp/mc admin info service >/dev/null 2>&1; then
    printf "object-storage service identity reached an admin API\n" >&2
    exit 1
  fi
  if mc --config-dir /tmp/mc ls "service/${forbidden_bucket}" >/dev/null 2>&1; then
    printf "object-storage service identity crossed the bucket boundary\n" >&2
    exit 1
  fi
  if mc --config-dir /tmp/mc ls "service/${bucket}/outside-prefix/" >/dev/null 2>&1; then
    printf "object-storage service identity listed outside subjects/\n" >&2
    exit 1
  fi
  printf "allowed" >/tmp/allowed-object
  mc --config-dir /tmp/mc cp \
    /tmp/allowed-object "service/${bucket}/subjects/ar02-policy-probe/allowed"
  mc --config-dir /tmp/mc ls \
    "service/${bucket}/subjects/ar02-policy-probe/" >/dev/null
  if mc --config-dir /tmp/mc cp \
    /tmp/allowed-object "service/${bucket}/outside-prefix/denied" >/dev/null 2>&1; then
    printf "object-storage service identity wrote outside subjects/\n" >&2
    exit 1
  fi
  mc --config-dir /tmp/mc rm --force \
    "service/${bucket}/subjects/ar02-policy-probe/allowed"
  mc --config-dir /tmp/mc rb --force "root/${forbidden_bucket}"
'
if curl --fail --silent --show-error \
  --cacert "$runtime_directory/wrong-ca.crt" \
  https://localhost:29000/minio/health/live >/dev/null 2>&1; then
  printf 'MinIO accepted an unrelated private CA\n' >&2
  exit 1
fi
if curl --fail --silent --show-error --noproxy '*' \
  --resolve ar02-wrong-host.invalid:29000:127.0.0.1 \
  --cacert "$runtime_directory/ca.crt" \
  https://ar02-wrong-host.invalid:29000/minio/health/live >/dev/null 2>&1; then
  printf 'MinIO TLS accepted the wrong hostname\n' >&2
  exit 1
fi
if curl --fail --silent http://localhost:29000/minio/health/live >/dev/null 2>&1; then
  printf 'MinIO accepted plaintext HTTP on its TLS endpoint\n' >&2
  exit 1
fi

minio_fingerprint_v1="$(
  openssl s_client \
    -connect localhost:29000 \
    -servername localhost \
    -CAfile "$runtime_directory/ca.crt" </dev/null 2>/dev/null |
    openssl x509 -noout -fingerprint -sha256
)"
issue_certificate minio_server_v2 minio serverAuth \
  "DNS:minio,DNS:localhost,IP:127.0.0.1"
copy_secret minio_tls_certificate "$runtime_directory/minio_server_v2.crt"
copy_secret minio_tls_private_key "$runtime_directory/minio_server_v2.key"
"${compose_command[@]}" up --detach --wait --wait-timeout 180 --force-recreate minio
"${compose_command[@]}" run --rm minio-init
minio_fingerprint_v2="$(
  openssl s_client \
    -connect localhost:29000 \
    -servername localhost \
    -CAfile "$runtime_directory/ca.crt" </dev/null 2>/dev/null |
    openssl x509 -noout -fingerprint -sha256
)"
if [[ "$minio_fingerprint_v1" == "$minio_fingerprint_v2" ]]; then
  printf 'MinIO certificate fingerprint did not change after rotation\n' >&2
  exit 1
fi
curl --fail --silent --show-error \
  --cacert "$runtime_directory/ca.crt" \
  https://localhost:29000/minio/health/live >/dev/null
