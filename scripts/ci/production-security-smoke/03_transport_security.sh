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
