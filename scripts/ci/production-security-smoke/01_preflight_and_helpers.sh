# Responsibility-focused phase sourced by production-security-smoke.sh.
cleanup() {
  local exit_code="$?"
  if [[ "$exit_code" -ne 0 ]]; then
    printf 'production security smoke failed; Compose diagnostics follow\n' >&2
    "${compose_command[@]}" ps --all >&2 || true
    "${compose_command[@]}" logs --no-color --tail 200 >&2 || true
  fi
  "${compose_command[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
  docker service rm "$swarm_service" >/dev/null 2>&1 || true
  docker secret rm "$swarm_secret_v1" "$swarm_secret_v2" >/dev/null 2>&1 || true
  if [[ "$initialized_swarm" == true ]]; then
    docker swarm leave --force >/dev/null 2>&1 || true
  fi
  rm -rf "$runtime_directory"
  return "$exit_code"
}
trap cleanup EXIT

if [[ "${GITHUB_ACTIONS:-}" != true ]]; then
  printf 'production security smoke requires an isolated GitHub Actions Docker daemon\n' >&2
  exit 1
fi

for required_command in \
  docker openssl psql curl python3 timeout sha256sum awk; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    printf 'production security smoke requires command: %s\n' "$required_command" >&2
    exit 1
  fi
done

# A hardened self-hosted runner may check out tracked files under umask 0077.
# These are public file-backed Compose configs (not credentials), and the
# container users must be able to read their bind mounts. Compose ignores
# per-config uid/gid/mode for local file sources.
chmod 0644 \
  "$root/config/nats/nats.conf" \
  "$root/config/nginx/trpg.conf" \
  "$root/config/postgres/001-security-roles.sql" \
  "$root/config/postgres/002-application-role.sh" \
  "$root/config/postgres/pg_hba.conf" \
  "$root/config/postgres/witness-runtime-roles.sh" \
  "$root/config/postgres/witness_pg_hba.conf" \
  "$root/config/redis/redis.conf" \
  "$root/policy/opa/security_governance.rego" \
  "$root/policy/openfga/security_governance.json" \
  "$root/scripts/ci/p02_policy_bootstrap.py"

install -d -m 0700 "$secret_directory"
certificate_serial=1000

issue_certificate() {
  local prefix="$1"
  local common_name="$2"
  local extended_usage="$3"
  local subject_alternative_name="$4"
  certificate_serial=$((certificate_serial + 1))
  openssl req -newkey rsa:2048 -nodes -sha256 \
    -subj "/CN=$common_name" \
    -keyout "$runtime_directory/$prefix.key" \
    -out "$runtime_directory/$prefix.csr" >/dev/null 2>&1
  {
    printf 'basicConstraints=critical,CA:FALSE\n'
    printf 'keyUsage=critical,digitalSignature,keyEncipherment\n'
    printf 'extendedKeyUsage=%s\n' "$extended_usage"
    printf 'subjectAltName=%s\n' "$subject_alternative_name"
  } >"$runtime_directory/$prefix.ext"
  openssl x509 -req -sha256 -days 1 \
    -in "$runtime_directory/$prefix.csr" \
    -CA "$runtime_directory/ca.crt" \
    -CAkey "$runtime_directory/ca.key" \
    -set_serial "$certificate_serial" \
    -extfile "$runtime_directory/$prefix.ext" \
    -out "$runtime_directory/$prefix.crt" >/dev/null 2>&1
  chmod 0600 "$runtime_directory/$prefix.key"
  chmod 0644 "$runtime_directory/$prefix.crt"
}

write_secret() {
  local name="$1"
  local value="$2"
  (
    umask 077
    printf '%s' "$value" >"$secret_directory/$name"
  )
}

copy_secret() {
  local name="$1"
  local source="$2"
  install -m 0600 "$source" "$secret_directory/$name"
}

witness_query() {
  local role="$1"
  local password="$2"
  local statement="$3"
  PGPASSWORD="$password" \
    PGSSLMODE=verify-full \
    PGSSLROOTCERT="$runtime_directory/ca.crt" \
    psql -X -A -t --set=ON_ERROR_STOP=1 \
      -h localhost -p 25433 -U "$role" -d coc_ai_trpg_witness \
      -c "$statement"
}

expect_witness_denied() {
  local role="$1"
  local password="$2"
  local operation="$3"
  local statement="$4"
  if witness_query "$role" "$password" "$statement" >/dev/null 2>&1; then
    printf 'PostgreSQL witness runtime role unexpectedly allowed %s: %s\n' \
      "$operation" "$role" >&2
    exit 1
  fi
}

wait_for_task_container() {
  local service_name="$1"
  local previous_container="${2:-}"
  local attempt
  local -a container_ids=()
  for ((attempt = 1; attempt <= 120; attempt++)); do
    mapfile -t container_ids < <(
      docker ps \
        --filter "label=com.docker.swarm.service.name=$service_name" \
        --filter status=running \
        --format '{{.ID}}'
    )
    if [[ "${#container_ids[@]}" -eq 1 &&
      "${container_ids[0]}" != "$previous_container" ]]; then
      printf '%s\n' "${container_ids[0]}"
      return 0
    fi
    sleep 0.5
  done
  printf 'timed out waiting for one running task container for %s\n' "$service_name" >&2
  docker service ps --no-trunc "$service_name" >&2 || true
  docker ps --all \
    --filter "label=com.docker.swarm.service.name=$service_name" >&2 || true
  return 1
}
