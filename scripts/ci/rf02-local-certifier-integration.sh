#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'
umask 077

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
for command_name in docker openssl python3 sha256sum; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'RF02 integration requires command: %s\n' "$command_name" >&2
    exit 2
  }
done
docker compose version >/dev/null

test_root="$(mktemp -d)"
project="rf02-certifier-$RANDOM-$$"
state="$test_root/state"
provider="$test_root/provider"
log="$test_root/bootstrap.log"
install -d -m 0700 "$provider"
export TRPG_CANONICAL_HMAC_KEY_ID="$project-canonical-v1"
export TRPG_PAYLOAD_ENCRYPTION_KEY_ID="$project-payload-v1"
export TRPG_AUDIT_HMAC_KEY_ID="$project-audit-v1"
export TRPG_LOCAL_MODEL_CERTIFICATION_HMAC_KEY_ID="$project-local-model-certification-v1"
export TRPG_OBJECT_STORAGE_BUCKET="trpg-$project"
export TRPG_MODEL_PROVIDER_TYPE="llama_cpp"
export TRPG_MODEL_PROVIDER_ID="$project-provider"
export TRPG_MODEL_ID="rf02-local-model"
export TRPG_MODEL_ARTIFACT_SHA256="sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
export TRPG_MODEL_PROVIDER_BASE_URL="https://127.0.0.1:9443/v1"
export TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID="$project-provider-route-v1"

cleanup() {
  local code="$?"
  if [[ -f "$state/runtime/compose.bootstrap.yml" ]]; then
    docker compose --project-name "$project" -f "$root/compose.yml" \
      -f "$provider/compose.yml" -f "$state/runtime/compose.bootstrap.yml" \
      --profile staged-worker --profile local-model-certifier \
      down --volumes --remove-orphans >/dev/null 2>&1 || true
  fi
  if [[ "$code" -ne 0 && "${RF02_KEEP_FAILED_EVIDENCE:-}" == 1 ]]; then
    printf 'RF02 failed evidence retained at %s\n' "$test_root" >&2
  else
    rm -rf "$test_root"
  fi
  return "$code"
}
trap cleanup EXIT

provider_canary='RF02_LOCAL_PROVIDER_CANARY_6a421f0b'
printf '%s' "$provider_canary" >"$provider/token"
openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 1 \
  -subj "/CN=RF02 Provider Root" -keyout "$provider/ca.key" \
  -out "$provider/ca.crt" >/dev/null 2>&1
openssl req -newkey rsa:2048 -nodes -sha256 -subj "/CN=127.0.0.1" \
  -keyout "$provider/server.key" -out "$provider/server.csr" >/dev/null 2>&1
printf 'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=IP:127.0.0.1\n' \
  >"$provider/server.ext"
openssl x509 -req -sha256 -days 1 -in "$provider/server.csr" \
  -CA "$provider/ca.crt" -CAkey "$provider/ca.key" -set_serial 2201 \
  -extfile "$provider/server.ext" -out "$provider/server.crt" >/dev/null 2>&1
chmod 0600 "$provider"/*.key "$provider/token"

cat >"$provider/server.py" <<'PY'
import http.server, json, ssl
TOKEN = open("/provider/token", encoding="utf-8").read()
class Handler(http.server.BaseHTTPRequestHandler):
    def record(self):
        with open("/provider/requests.log", "a", encoding="utf-8") as log:
            log.write(self.command + " " + self.path + "\n")
    def authorized(self):
        return self.headers.get("Authorization") == "Bearer " + TOKEN
    def do_GET(self):
        self.record()
        if self.path != "/v1/models" or not self.authorized():
            self.send_response(403); self.end_headers(); return
        body = json.dumps({"data": [{"id": "rf02-local-model"}]}).encode()
        self.send_response(200); self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def do_POST(self):
        self.record()
        self.send_response(503); self.end_headers()
    def log_message(self, *_args):
        return
server = http.server.ThreadingHTTPServer(("127.0.0.1", 9443), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain("/provider/server.crt", "/provider/server.key")
server.socket = context.wrap_socket(server.socket, server_side=True)
server.serve_forever()
PY
cat >"$provider/compose.yml" <<YAML
services:
  rf02-local-provider:
    image: python:3.13-alpine@sha256:399babc8b49529dabfd9c922f2b5eea81d611e4512e3ed250d75bd2e7683f4b0
    command: ["python3", "/provider/server.py"]
    read_only: true
    security_opt: ["no-new-privileges:true"]
    network_mode: "service:openfga"
    volumes:
      - $provider:/provider
YAML

bootstrap=(
  "$root/scripts/bootstrap/bootstrap.sh"
  --state-dir "$state"
  --project-name "$project"
  --provider-type llama_cpp
  --provider-url https://127.0.0.1:9443/v1
  --provider-model rf02-local-model
  --provider-sha256 sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  --provider-runtime-sha256 sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  --provider-credential-file "$provider/token"
  --provider-ca-file "$provider/ca.crt"
  --extra-compose-file "$provider/compose.yml"
)

TRPG_BOOTSTRAP_TEST_STOP_AFTER_STEP=model_certification_request \
  "${bootstrap[@]}" >>"$log" 2>&1 &
bootstrap_pid="$!"
deadline=$((SECONDS + 1800))
while kill -0 "$bootstrap_pid" 2>/dev/null; do
  process_state="$(ps -o stat= -p "$bootstrap_pid" 2>/dev/null || true)"
  [[ "$process_state" == *T* ]] && break
  ((SECONDS < deadline)) || {
    printf 'timed out waiting for certification request checkpoint\n' >&2
    kill -KILL "$bootstrap_pid" 2>/dev/null || true
    exit 1
  }
  sleep 0.1
done
[[ "$(ps -o stat= -p "$bootstrap_pid" 2>/dev/null || true)" == *T* ]] || {
  tail -n 160 "$log" >&2
  printf 'bootstrap did not stop after certification request\n' >&2
  exit 1
}
kill -KILL "$bootstrap_pid"
wait "$bootstrap_pid" 2>/dev/null || true
awk -F $'\t' '$2 == "model_certification_request" && $3 == "OK" {found=1} END {exit !found}' \
  "$state/state.tsv"
printf 'RF02 checkpoint captured project=%s\n' "$project"

set +e
"${bootstrap[@]}" >>"$log" 2>&1
bootstrap_status="$?"
set -e
[[ "$bootstrap_status" -eq 6 ]] || {
  tail -n 120 "$log" >&2
  printf 'expected terminal certification failure, status=%s\n' "$bootstrap_status" >&2
  exit 1
}
grep -F 'bootstrap error=MODEL_CERTIFICATION_TERMINAL_FAILURE' "$log" >/dev/null
grep -E '^POST /v1/(chat/completions|embeddings)$' "$provider/requests.log" >/dev/null
printf 'RF02 terminal failure observed project=%s\n' "$project"

compose=(docker compose --project-name "$project" -f "$root/compose.yml" \
  -f "$provider/compose.yml" -f "$state/runtime/compose.bootstrap.yml" \
  --profile staged-worker --profile local-model-certifier)
"${compose[@]}" ps --status running --services | grep -Fx local-model-certifier >/dev/null
if "${compose[@]}" ps --status running --services | grep -Fx agent-worker >/dev/null; then
  printf 'uncertified agent worker started after failed certification\n' >&2
  exit 1
fi
request_id="$project-model-certification"
result_path="/var/lib/trpg/local-model-certification/$request_id.result.json"
status_path="/var/lib/trpg/model-certification-status/$request_id.status.json"
"${compose[@]}" exec -T local-model-certifier /bin/cat "$result_path" >"$test_root/result-before.json"
"${compose[@]}" exec -T local-model-certifier /bin/cat "$status_path" >"$test_root/status-before.json"
"${compose[@]}" exec -T local-model-certifier /bin/sh -ec \
  'test ! -e /var/lib/trpg/local-model-certification/certificate.json; if test -e /var/lib/trpg/local-model-certification/registry.jsonl; then cat /var/lib/trpg/local-model-certification/registry.jsonl; fi' \
  >"$test_root/registry-before.jsonl"
[[ ! -s "$test_root/registry-before.jsonl" ]]
python3 - "$test_root/result-before.json" "$test_root/status-before.json" "$request_id" "$result_path" <<'PY'
import json, sys
result = json.load(open(sys.argv[1], encoding="utf-8"))
status = json.load(open(sys.argv[2], encoding="utf-8"))
request_id, result_path = sys.argv[3:]
assert result["request_id"] == request_id and result["state"] == "failed"
assert result["attempt"] == 1 and result["certificate_path"] is None
assert status == {
    "schema_version": 1,
    "request_id": request_id,
    "state": "failed",
    "result_reference": result_path,
    "certificate_reference": None,
    "error_code": result["error_code"],
}
PY

set +e
"${bootstrap[@]}" >>"$log" 2>&1
rerun_status="$?"
set -e
[[ "$rerun_status" -eq 6 ]]
"${compose[@]}" exec -T local-model-certifier /bin/cat "$result_path" >"$test_root/result-after.json"
"${compose[@]}" exec -T local-model-certifier /bin/cat "$status_path" >"$test_root/status-after.json"
"${compose[@]}" exec -T local-model-certifier /bin/sh -ec \
  'test ! -e /var/lib/trpg/local-model-certification/certificate.json; if test -e /var/lib/trpg/local-model-certification/registry.jsonl; then cat /var/lib/trpg/local-model-certification/registry.jsonl; fi' \
  >"$test_root/registry-after.jsonl"
cmp -s "$test_root/result-before.json" "$test_root/result-after.json"
cmp -s "$test_root/status-before.json" "$test_root/status-after.json"
cmp -s "$test_root/registry-before.jsonl" "$test_root/registry-after.jsonl"
if awk -F $'\t' '$2 == "model_certification" || $2 == "agent_worker_ready" {found=1} END {exit !found}' \
  "$state/state.tsv"; then
  printf 'failed certification advanced the bootstrap journal\n' >&2
  exit 1
fi
printf 'RF02 idempotent rerun verified project=%s\n' "$project"

printf 'RF02 local certifier integration passed project=%s result_sha256=%s status_sha256=%s\n' \
  "$project" "$(sha256sum "$test_root/result-after.json" | awk '{print $1}')" \
  "$(sha256sum "$test_root/status-after.json" | awk '{print $1}')"
