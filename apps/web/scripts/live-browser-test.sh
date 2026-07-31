#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'
umask 077

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
for command_name in docker openssl node python3; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'AR11 live browser test requires command: %s\n' "$command_name" >&2
    exit 2
  }
done
docker compose version >/dev/null

test_root="$(mktemp -d)"
project="ar11-$RANDOM-$$"
state="$test_root/state"
provider="$test_root/provider"
bootstrap_log="$test_root/bootstrap.log"
evidence="${AR11_EVIDENCE_DIR:-$(mktemp -d -t ar11-live-evidence-XXXXXX)}"
[[ "$evidence" = /* && ! -L "$evidence" ]] || {
  printf 'AR11_EVIDENCE_DIR must be an absolute non-symlink path\n' >&2
  exit 2
}
install -d -m 0700 "$provider" "$evidence"

export AR11_REPOSITORY_ROOT="$repository_root"
export AR11_PROVIDER_DIRECTORY="$provider"
export AR11_MODEL_ID="ar11-browser-model"
export AR11_PRIVATE_CANARY="AR11_PRIVATE_CANARY_$(openssl rand -hex 16)"
export TRPG_CANONICAL_HMAC_KEY_ID="$project-canonical-v1"
export TRPG_PAYLOAD_ENCRYPTION_KEY_ID="$project-payload-v1"
export TRPG_AUDIT_HMAC_KEY_ID="$project-audit-v1"
export TRPG_OBJECT_STORAGE_BUCKET="trpg-$project"
export TRPG_MODEL_PROVIDER_TYPE="cloud"
export TRPG_MODEL_PROVIDER_ID="$project-provider"
export TRPG_MODEL_ID="$AR11_MODEL_ID"
export TRPG_MODEL_ARTIFACT_SHA256="sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
export TRPG_MODEL_PROVIDER_BASE_URL="https://provider.test:9443/v1"
export TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID="$project-provider-route-v1"

compose=(
  docker compose --project-name "$project"
  -f "$repository_root/compose.yml"
  -f "$repository_root/apps/web/scripts/ar11-provider.compose.yml"
)

cleanup() {
  local code="$?"
  if [[ "$code" -ne 0 && "${AR11_KEEP_FAILED_STACK:-}" == 1 ]]; then
    install -m 0600 "$bootstrap_log" "$evidence/bootstrap.log" 2>/dev/null || true
    printf 'AR11 failed stack retained project=%s root=%s evidence=%s\n' \
      "$project" "$test_root" "$evidence" >&2
    return "$code"
  fi
  if [[ -f "$state/runtime/compose.bootstrap.yml" ]]; then
    if [[ "$code" -ne 0 ]]; then
      "${compose[@]}" -f "$state/runtime/compose.bootstrap.yml" logs --no-color \
        >"$evidence/compose.log" 2>&1 || true
      install -m 0600 "$bootstrap_log" "$evidence/bootstrap.log" 2>/dev/null || true
    fi
    "${compose[@]}" -f "$state/runtime/compose.bootstrap.yml" \
      down --volumes --remove-orphans >/dev/null 2>&1 || true
  fi
  rm -rf "$test_root"
  if [[ "$code" -ne 0 ]]; then
    printf 'AR11 live browser evidence retained at %s\n' "$evidence" >&2
  fi
  return "$code"
}
trap cleanup EXIT

printf '%s' "$(openssl rand -hex 24)" >"$provider/token"
printf '%s' "$AR11_PRIVATE_CANARY" >"$test_root/private-canary"
openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 1 \
  -subj "/CN=AR11 Provider Root" -keyout "$provider/ca.key" \
  -out "$provider/ca.crt" >/dev/null 2>&1
openssl req -newkey rsa:2048 -nodes -sha256 -subj "/CN=provider.test" \
  -keyout "$provider/server.key" -out "$provider/server.csr" >/dev/null 2>&1
printf '%s\n' \
  'basicConstraints=critical,CA:FALSE' \
  'keyUsage=critical,digitalSignature,keyEncipherment' \
  'extendedKeyUsage=serverAuth' \
  'subjectAltName=DNS:provider.test' >"$provider/server.ext"
openssl x509 -req -sha256 -days 1 -in "$provider/server.csr" \
  -CA "$provider/ca.crt" -CAkey "$provider/ca.key" -set_serial 2011 \
  -extfile "$provider/server.ext" -out "$provider/server.crt" >/dev/null 2>&1
chmod 0600 "$provider"/*.key "$provider/token"

"$repository_root/scripts/bootstrap/bootstrap.sh" \
  --state-dir "$state" \
  --project-name "$project" \
  --provider-type openai \
  --provider-url https://provider.test:9443/v1 \
  --provider-model "$AR11_MODEL_ID" \
  --provider-sha256 "$TRPG_MODEL_ARTIFACT_SHA256" \
  --provider-credential-file "$provider/token" \
  --provider-ca-file "$provider/ca.crt" \
  --extra-compose-file "$repository_root/apps/web/scripts/ar11-provider.compose.yml" \
  >"$bootstrap_log" 2>&1

runtime_compose=("${compose[@]}" -f "$state/runtime/compose.bootstrap.yml")
golden_capabilities=(Web API Realtime Agent Provider Export)
golden_services=(web api realtime agent-worker ar11-provider agent-worker)

require_golden_capabilities() {
  local running_services index capability service
  running_services="$("${runtime_compose[@]}" ps --status running --services)"
  for index in "${!golden_capabilities[@]}"; do
    capability="${golden_capabilities[$index]}"
    service="${golden_services[$index]}"
    if ! grep -Fxq "$service" <<<"$running_services"; then
      printf 'required Golden capability is absent: %s (service=%s)\n' \
        "$capability" "$service" >&2
      return 1
    fi
  done
}

wait_for_service() {
  local service="$1" container state attempt
  for attempt in {1..90}; do
    container="$("${runtime_compose[@]}" ps -q "$service")"
    if [[ -n "$container" ]]; then
      state="$(docker inspect --format \
        '{{.State.Status}}|{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' \
        "$container")"
      if [[ "$state" == "running|healthy" || "$state" == "running|none" ]]; then
        return 0
      fi
    fi
    sleep 1
  done
  printf 'Golden service did not recover: %s\n' "$service" >&2
  return 1
}

require_golden_capabilities

certificate_spki="$(
  openssl x509 -in "$state/runtime/reverse_proxy.crt" -pubkey -noout \
    | openssl pkey -pubin -outform DER 2>/dev/null \
    | openssl dgst -sha256 -binary \
    | openssl base64 -A
)"

AR11_CERTIFICATE_SPKI="$certificate_spki" \
AR11_LIVE_ORIGIN="https://127.0.0.1:8443" \
AR11_CREDENTIALS_FILE="$state/credentials/initial-accounts.env" \
AR11_TUTORIAL_FILE="$state/credentials/tutorial.env" \
AR11_EVIDENCE_DIRECTORY="$evidence" \
node "$repository_root/apps/web/scripts/live-browser-test.mjs"

component_gate_log="$evidence/component-failure-gates.log"
: >"$component_gate_log"
for index in "${!golden_capabilities[@]}"; do
  capability="${golden_capabilities[$index]}"
  service="${golden_services[$index]}"
  printf 'INJECT capability=%s service=%s\n' "$capability" "$service" \
    >>"$component_gate_log"
  "${runtime_compose[@]}" stop "$service" >>"$component_gate_log" 2>&1
  if require_golden_capabilities >>"$component_gate_log" 2>&1; then
    printf 'Golden incorrectly passed without %s (%s)\n' "$capability" "$service" >&2
    exit 1
  fi
  printf 'EXPECTED_FAIL capability=%s service=%s\n' "$capability" "$service" \
    >>"$component_gate_log"
  "${runtime_compose[@]}" start "$service" >>"$component_gate_log" 2>&1
  wait_for_service "$service"
  require_golden_capabilities
  printf 'RECOVERED capability=%s service=%s\n' "$capability" "$service" \
    >>"$component_gate_log"
done

install -m 0600 "$bootstrap_log" "$evidence/bootstrap.log"
"${runtime_compose[@]}" ps --format json \
  >"$evidence/compose-ps.json"
chmod 0600 "$evidence"/*
printf 'AR11 live browser test passed project=%s evidence=%s\n' "$project" "$evidence"
