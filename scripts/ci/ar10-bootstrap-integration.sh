#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'
umask 077

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
for command_name in docker openssl curl python3 timeout; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'AR10 integration requires command: %s\n' "$command_name" >&2
    exit 2
  }
done
docker compose version >/dev/null
test_root="$(mktemp -d)"
project="ar10-$RANDOM-$$"
state="$test_root/state"
provider="$test_root/provider"
log="$test_root/bootstrap.log"
install -d -m 0700 "$provider"
export TRPG_CANONICAL_HMAC_KEY_ID="$project-canonical-v1"
export TRPG_PAYLOAD_ENCRYPTION_KEY_ID="$project-payload-v1"
export TRPG_AUDIT_HMAC_KEY_ID="$project-audit-v1"
export TRPG_OBJECT_STORAGE_BUCKET="trpg-$project"
export TRPG_MODEL_PROVIDER_TYPE="cloud"
export TRPG_MODEL_PROVIDER_ID="$project-provider"
export TRPG_MODEL_ID="ar10-exact-model"
export TRPG_MODEL_ARTIFACT_SHA256="sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
export TRPG_MODEL_PROVIDER_BASE_URL="https://provider.test:9443/v1"
export TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID="$project-provider-route-v1"
compose=(docker compose --project-name "$project" -f "$root/compose.yml")

cleanup() {
  local code="$?"
  if [[ -f "$state/runtime/compose.bootstrap.yml" ]]; then
    docker compose --project-name "$project" -f "$root/compose.yml" \
      -f "$provider/compose.yml" -f "$state/runtime/compose.bootstrap.yml" \
      down --volumes --remove-orphans >/dev/null 2>&1 || true
  fi
  if [[ "$code" -ne 0 && "${AR10_KEEP_FAILED_EVIDENCE:-}" == 1 ]]; then
    printf 'AR10 failed evidence retained at %s\n' "$test_root" >&2
  else
    rm -rf "$test_root"
  fi
  return "$code"
}
trap cleanup EXIT

provider_canary='AR10_PROVIDER_SECRET_CANARY_7f447fbb'
printf '%s' "$provider_canary" >"$provider/token"
openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 1 \
  -subj "/CN=AR10 Provider Root" -keyout "$provider/ca.key" \
  -out "$provider/ca.crt" >/dev/null 2>&1
openssl req -newkey rsa:2048 -nodes -sha256 -subj "/CN=provider.test" \
  -keyout "$provider/server.key" -out "$provider/server.csr" >/dev/null 2>&1
printf 'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:provider.test\n' \
  >"$provider/server.ext"
openssl x509 -req -sha256 -days 1 -in "$provider/server.csr" \
  -CA "$provider/ca.crt" -CAkey "$provider/ca.key" -set_serial 2001 \
  -extfile "$provider/server.ext" -out "$provider/server.crt" >/dev/null 2>&1
chmod 0600 "$provider"/*.key "$provider/token"
cat >"$provider/server.py" <<'PY'
import http.server, json, ssl
TOKEN = open("/provider/token", encoding="utf-8").read()
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/v1/models" or self.headers.get("Authorization") != "Bearer " + TOKEN:
            self.send_response(403); self.end_headers(); return
        body = json.dumps({"data": [{"id": "ar10-exact-model"}]}).encode()
        self.send_response(200); self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *_args):
        return
server = http.server.ThreadingHTTPServer(("0.0.0.0", 9443), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain("/provider/server.crt", "/provider/server.key")
server.socket = context.wrap_socket(server.socket, server_side=True)
server.serve_forever()
PY
cat >"$provider/compose.yml" <<YAML
services:
  ar10-provider:
    image: python:3.13-alpine@sha256:399babc8b49529dabfd9c922f2b5eea81d611e4512e3ed250d75bd2e7683f4b0
    command: ["python3", "/provider/server.py"]
    read_only: true
    security_opt: ["no-new-privileges:true"]
    volumes:
      - $provider:/provider:ro
    networks:
      backend:
        aliases: [provider.test]
YAML

bootstrap=(
  "$root/scripts/bootstrap/bootstrap.sh"
  --state-dir "$state"
  --project-name "$project"
  --provider-type openai
  --provider-url https://provider.test:9443/v1
  --provider-model ar10-exact-model
  --provider-sha256 sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  --provider-credential-file "$provider/token"
  --provider-ca-file "$provider/ca.crt"
  --extra-compose-file "$provider/compose.yml"
)
bad_bootstrap=("${bootstrap[@]}")
bad_bootstrap[2]="$test_root/bad-state"
bad_bootstrap[4]="$project-bad"
bad_bootstrap[8]="http://provider.test:9443/v1"
if "${bad_bootstrap[@]}" >"$test_root/bad-prerequisite.log" 2>&1; then
  printf 'bootstrap accepted an insecure provider prerequisite\n' >&2
  exit 1
fi
grep -F 'bootstrap error=PROVIDER_URL_INVALID' "$test_root/bad-prerequisite.log" >/dev/null
steps=(preflight secrets compose_config compose_up recovery_roles admin_bootstrap provider_configure provider_probe model_certification self_check)
for step in "${steps[@]}"; do
  TRPG_BOOTSTRAP_TEST_STOP_AFTER_STEP="$step" "${bootstrap[@]}" >>"$log" 2>&1 &
  pid="$!"
  deadline=$((SECONDS + 1800))
  while kill -0 "$pid" 2>/dev/null; do
    process_state="$(ps -o stat= -p "$pid" 2>/dev/null || true)"
    [[ "$process_state" == *T* ]] && break
    ((SECONDS < deadline)) || {
      printf 'timed out waiting to kill bootstrap after %s\n' "$step" >&2
      kill -KILL "$pid" 2>/dev/null || true
      exit 1
    }
    sleep 0.1
  done
  process_state="$(ps -o stat= -p "$pid" 2>/dev/null || true)"
  [[ "$process_state" == *T* ]] || {
    wait "$pid" || status="$?"
    tail -n 120 "$log" >&2
    printf 'bootstrap process exited before stop step=%s status=%s\n' "$step" "${status:-0}" >&2
    printf 'bootstrap did not stop after committed step %s\n' "$step" >&2
    exit 1
  }
  kill -KILL "$pid"
  wait "$pid" 2>/dev/null || true
  awk -F $'\t' -v expected="$step" '$2 == expected && $3 == "OK" {found=1} END {exit !found}' \
    "$state/state.tsv"
done
"${bootstrap[@]}" >>"$log" 2>&1

compose+=(-f "$provider/compose.yml" -f "$state/runtime/compose.bootstrap.yml")
credentials="$state/credentials/initial-accounts.env"
# shellcheck disable=SC1090
source "$credentials"
scratch="$test_root/api"
install -d -m 0700 "$scratch"
api='https://127.0.0.1:8443/admin/v1'
response="$scratch/response.json"
api_call() {
  local method="$1" path="$2" headers="$3" body="$4"
  local -a args=(--silent --show-error --cacert "$state/runtime/ca.crt" --request "$method" --header "@$headers" --output "$response" --write-out '%{http_code}')
  [[ -z "$body" ]] || args+=(--header 'Content-Type: application/json' --data-binary "@$body")
  curl "${args[@]}" "$api/$path"
}
json_value() { python3 - "$response" "$1" <<'PY'
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
for part in sys.argv[2].split("."): value = value[part]
print(str(value).lower() if isinstance(value, bool) else value)
PY
}
python3 - "$credentials" "$scratch/login.json" "$scratch/business-login.json" <<'PY'
import json, shlex, sys
values = {}
for line in open(sys.argv[1], encoding="utf-8"):
    key, value = line.rstrip("\n").split("=", 1); values[key] = shlex.split(value)[0]
json.dump({"login": values["ADMIN_LOGIN"], "password": values["ADMIN_PASSWORD"]}, open(sys.argv[2], "w"))
json.dump({"login": values["BUSINESS_LOGIN"], "password": values["BUSINESS_PASSWORD"]}, open(sys.argv[3], "w"))
PY
printf 'Accept: application/json\n' >"$scratch/public.headers"
[[ "$(api_call POST sessions "$scratch/public.headers" "$scratch/login.json")" == 200 ]]
printf 'Authorization: Bearer %s\nAccept: application/json\n' "$(json_value access_token)" >"$scratch/owner.headers"
printf 'Authorization: Bearer %s\nAccept: application/json\n' \
  "$(<"$state/runtime/secrets/admin_bootstrap_token")" >"$scratch/bootstrap.headers"
[[ "$(api_call GET bootstrap/status "$scratch/bootstrap.headers" '')" == 401 ]]
[[ "$(api_call POST sessions "$scratch/public.headers" "$scratch/business-login.json")" == 403 ]]
[[ "$(api_call GET bootstrap/status "$scratch/owner.headers" '')" == 200 ]]
[[ "$(json_value administrator_count)" == 1 && "$(json_value business_account_count)" == 1 ]]

mutation_headers() {
  local version="$1" key="$2"
  { cat "$scratch/owner.headers"; printf 'Idempotency-Key: %s\nX-Expected-Version: %s\nX-Correlation-Id: %s\nX-Causation-Id: ar10-integration\n' "$key" "$version" "$key"; } >"$scratch/mutation.headers"
}
version="$(json_value state_version)"
printf '{"backup_id":"ar10_backup","schema_version":"ar10_schema_v1"}' >"$scratch/backup.json"
mutation_headers "$version" ar10-backup
[[ "$(api_call POST backups "$scratch/mutation.headers" "$scratch/backup.json")" == 200 ]]
manifest="$(json_value artifact_reference)"
[[ "$(json_value digest)" =~ ^sha256:[0-9a-f]{64}$ ]]

"${compose[@]}" exec -T postgres sh -ec \
  'export PGPASSWORD="$(cat /run/secrets/postgres_restore_password)"; exec psql -X -q -v ON_ERROR_STOP=1 -U trpg_restore_login -d coc_ai_trpg_restore' \
  <<'SQL'
CREATE TABLE ar10_restore_marker(value TEXT NOT NULL);
INSERT INTO ar10_restore_marker(value) VALUES ('must disappear');
SQL
[[ "$(api_call GET bootstrap/status "$scratch/owner.headers" '')" == 200 ]]
version="$(json_value state_version)"
python3 - "$manifest" "$scratch/restore.json" <<'PY'
import json, sys
json.dump({"manifest_path": sys.argv[1], "expected_schema_version": "ar10_schema_v1",
           "safety_point_id": "ar10_before_restore"}, open(sys.argv[2], "w"))
PY
mutation_headers "$version" ar10-restore
[[ "$(api_call POST restores "$scratch/mutation.headers" "$scratch/restore.json")" == 200 ]]
marker_absent="$("${compose[@]}" exec -T postgres sh -ec \
  'export PGPASSWORD="$(cat /run/secrets/postgres_restore_password)";
   exec psql -X -A -t -v ON_ERROR_STOP=1 -U trpg_restore_login -d coc_ai_trpg_restore' <<'SQL'
SELECT to_regclass('public.ar10_restore_marker') IS NULL;
SQL
)"
[[ "$marker_absent" == t ]]
primary_events="$("${compose[@]}" exec -T postgres sh -ec \
  'export PGPASSWORD="$(cat /run/secrets/postgres_bootstrap_password)"; psql -X -A -t -U trpg_database_owner -d coc_ai_trpg -c "SELECT count(*) FROM event_store"')"
restore_events="$("${compose[@]}" exec -T postgres sh -ec \
  'export PGPASSWORD="$(cat /run/secrets/postgres_restore_password)"; psql -X -A -t -U trpg_restore_login -d coc_ai_trpg_restore -c "SELECT count(*) FROM event_store"')"
[[ "$primary_events" == "$restore_events" ]]
restore_role_minimal="$("${compose[@]}" exec -T postgres sh -ec \
  'export PGPASSWORD="$(cat /run/secrets/postgres_bootstrap_password)";
   exec psql -X -A -t -v ON_ERROR_STOP=1 -U trpg_database_owner -d postgres' <<'SQL'
SELECT NOT rolsuper
   AND NOT rolcreatedb
   AND NOT rolcreaterole
   AND NOT rolreplication
   AND NOT rolbypassrls
  FROM pg_roles
 WHERE rolname = 'trpg_restore_login';
SQL
)"
[[ "$restore_role_minimal" == t ]]

[[ "$(api_call GET bootstrap/status "$scratch/owner.headers" '')" == 200 ]]
version="$(json_value state_version)"
python3 - "$manifest" "$scratch/bad-schema.json" <<'PY'
import json, sys
json.dump({"manifest_path": sys.argv[1], "expected_schema_version": "wrong_schema",
           "safety_point_id": "ar10_bad_schema"}, open(sys.argv[2], "w"))
PY
mutation_headers "$version" ar10-bad-schema
[[ "$(api_call POST restores "$scratch/mutation.headers" "$scratch/bad-schema.json")" == 503 ]]
"${compose[@]}" exec --user 10001:10001 -T admin sh -ec \
  'printf tamper >> /var/lib/trpg/backups/ar10_backup.dump'
python3 - "$manifest" "$scratch/bad-hash.json" <<'PY'
import json, sys
json.dump({"manifest_path": sys.argv[1], "expected_schema_version": "ar10_schema_v1",
           "safety_point_id": "ar10_bad_hash"}, open(sys.argv[2], "w"))
PY
mutation_headers "$version" ar10-bad-hash
[[ "$(api_call POST restores "$scratch/mutation.headers" "$scratch/bad-hash.json")" == 503 ]]

journal_hash="$(sha256sum "$state/state.tsv" | awk '{print $1}')"
"${bootstrap[@]}" >>"$log" 2>&1
[[ "$journal_hash" == "$(sha256sum "$state/state.tsv" | awk '{print $1}')" ]]
[[ ! -e "$state/runtime/bootstrap-scratch" ]]
"${compose[@]}" logs --no-color >"$test_root/compose.log" 2>&1
! grep -F "$provider_canary" "$log" "$test_root/compose.log"
! grep -F "$ADMIN_PASSWORD" "$log" "$test_root/compose.log"
! grep -F "$BUSINESS_PASSWORD" "$log" "$test_root/compose.log"
! "${compose[@]}" exec -T admin sh -ec \
  "grep -R -F '$provider_canary' /var/lib/trpg >/dev/null 2>&1"
if [[ -n "${AR10_EVIDENCE_DIR:-}" ]]; then
  [[ "$AR10_EVIDENCE_DIR" = /* && ! -L "$AR10_EVIDENCE_DIR" ]] || {
    printf 'AR10_EVIDENCE_DIR must be an absolute non-symlink path\n' >&2
    exit 2
  }
  install -d -m 0700 "$AR10_EVIDENCE_DIR"
  install -m 0600 "$log" "$AR10_EVIDENCE_DIR/bootstrap.log"
  install -m 0600 "$test_root/compose.log" "$AR10_EVIDENCE_DIR/compose.log"
  install -m 0600 "$test_root/bad-prerequisite.log" "$AR10_EVIDENCE_DIR/bad-prerequisite.log"
  install -m 0600 "$state/state.tsv" "$AR10_EVIDENCE_DIR/state.tsv"
  "${compose[@]}" exec -T admin cat "$manifest" >"$AR10_EVIDENCE_DIR/backup-manifest.json"
  "${compose[@]}" exec -T admin cat /var/lib/trpg/audit/admin.jsonl >"$AR10_EVIDENCE_DIR/admin-audit.jsonl"
  "${compose[@]}" ps --format json >"$AR10_EVIDENCE_DIR/compose-ps.json"
  printf 'project=%s\nsteps=%s\nresult=PASS\n' "$project" "${#steps[@]}" \
    >"$AR10_EVIDENCE_DIR/summary.txt"
  chmod 0600 "$AR10_EVIDENCE_DIR"/*
fi
printf 'AR10 bootstrap integration passed project=%s steps=%s\n' "$project" "${#steps[@]}"
