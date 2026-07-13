#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

if [[ "${TRPG_SKIP_BUILD:-0}" != "1" ]]; then
  cargo build --workspace --all-targets --release --locked
  pnpm --filter ./apps/web... build
fi

tmp_dir="$(mktemp -d)"
running_pid=""
if command -v python3 >/dev/null 2>&1; then
  python=python3
elif command -v python >/dev/null 2>&1; then
  python=python
else
  echo "Python is required for health contract validation" >&2
  exit 1
fi
cleanup() {
  if [[ -n "$running_pid" ]]; then
    kill "$running_pid" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

assert_health_json() {
  local file="$1"
  local service="$2"
  local status="$3"
  "$python" -c '
import json
import sys

payload = json.load(open(sys.argv[1], encoding="utf-8"))
assert payload["service"] == sys.argv[2], payload
assert payload["status"] == sys.argv[3], payload
assert isinstance(payload.get("version"), str) and payload["version"], payload
assert isinstance(payload.get("checks"), list) and payload["checks"], payload
assert all(check.get("status") in {"pass", "fail"} for check in payload["checks"]), payload
' "$file" "$service" "$status"
}

binary_path() {
  local name="$1"
  local release_dir="${CARGO_TARGET_DIR:-target}/release"
  if [[ -x "$release_dir/$name" ]]; then
    printf '%s\n' "$release_dir/$name"
  elif [[ -x "$release_dir/$name.exe" ]]; then
    printf '%s\n' "$release_dir/$name.exe"
  else
    echo "release binary is missing: $name" >&2
    return 1
  fi
}

smoke_service() {
  local service="$1"
  local port="$2"
  local binary
  local base_url="http://127.0.0.1:$port"
  local log="$tmp_dir/$service.log"
  local live_body="$tmp_dir/$service-live.json"
  local live_headers="$tmp_dir/$service-live.headers"
  binary="$(binary_path "$service")"

  TRPG_BIND_ADDR="127.0.0.1:$port" TRPG_MAX_REQUESTS=4 "$binary" >"$log" 2>&1 &
  running_pid=$!

  local live_code=""
  for _ in {1..50}; do
    if ! kill -0 "$running_pid" >/dev/null 2>&1; then
      cat "$log" >&2
      echo "$service exited before accepting requests" >&2
      return 1
    fi
    live_code="$(curl -sS -D "$live_headers" -o "$live_body" -w '%{http_code}' "$base_url/health/live" || true)"
    if [[ "$live_code" == "200" ]]; then
      break
    fi
    sleep 0.1
  done
  [[ "$live_code" == "200" ]]
  assert_health_json "$live_body" "$service" "live"
  grep -qi '^Access-Control-Allow-Origin: \*' "$live_headers"
  grep -qi '^Cache-Control: no-store' "$live_headers"

  local ready_body="$tmp_dir/$service-ready.json"
  local ready_code
  ready_code="$(curl -sS -o "$ready_body" -w '%{http_code}' "$base_url/health/ready")"
  [[ "$ready_code" == "200" ]]
  assert_health_json "$ready_body" "$service" "ready"

  local missing_body="$tmp_dir/$service-missing.json"
  local missing_code
  missing_code="$(curl -sS -o "$missing_body" -w '%{http_code}' "$base_url/not-a-health-route")"
  [[ "$missing_code" == "404" ]]
  assert_health_json "$missing_body" "$service" "not_found"

  local method_body="$tmp_dir/$service-method.json"
  local method_headers="$tmp_dir/$service-method.headers"
  local method_code
  method_code="$(curl -sS -X POST -D "$method_headers" -o "$method_body" -w '%{http_code}' "$base_url/health/live")"
  [[ "$method_code" == "405" ]]
  assert_health_json "$method_body" "$service" "method_not_allowed"
  grep -qi '^Allow: GET' "$method_headers"

  wait "$running_pid"
  running_pid=""
  printf 'service process smoke passed: %s\n' "$service"
}

smoke_service api-server 18080
smoke_service realtime-server 18081
smoke_service agent-worker 18082
smoke_service admin-server 18083
smoke_service migration-runner 18084

# Negative gate: a zero request limit used to permit an unbounded process. The
# runtime must now reject it before binding a listener.
api_binary="$(binary_path api-server)"
if TRPG_BIND_ADDR="127.0.0.1:18090" TRPG_MAX_REQUESTS=0 "$api_binary" >"$tmp_dir/invalid-config.log" 2>&1; then
  echo "api-server accepted an invalid zero request limit" >&2
  exit 1
fi
grep -q 'TRPG_MAX_REQUESTS must be a positive integer' "$tmp_dir/invalid-config.log"
echo "negative process configuration smoke passed"

WEB_HOST=127.0.0.1 WEB_PORT=18100 WEB_MAX_REQUESTS=2 \
  node apps/web/scripts/server.mjs --root apps/web/dist --port 18100 \
  >"$tmp_dir/web.log" 2>&1 &
running_pid=$!

web_index_code=""
for _ in {1..50}; do
  if ! kill -0 "$running_pid" >/dev/null 2>&1; then
    cat "$tmp_dir/web.log" >&2
    echo "web process exited before accepting requests" >&2
    exit 1
  fi
  web_index_code="$(curl -sS -o "$tmp_dir/web-index.html" -w '%{http_code}' http://127.0.0.1:18100/ || true)"
  if [[ "$web_index_code" == "200" ]]; then
    break
  fi
  sleep 0.1
done
[[ "$web_index_code" == "200" ]]
grep -q '<main' "$tmp_dir/web-index.html"

web_config_code="$(curl -sS -o "$tmp_dir/web-config.json" -w '%{http_code}' http://127.0.0.1:18100/app.config.json)"
[[ "$web_config_code" == "200" ]]
"$python" -c '
import json
import sys

payload = json.load(open(sys.argv[1], encoding="utf-8"))
services = payload.get("services")
assert isinstance(services, list) and len(services) == 5, payload
assert {service["processName"] for service in services} == {
    "api-server", "realtime-server", "agent-worker", "admin-server", "migration-runner"
}, payload
' "$tmp_dir/web-config.json"

wait "$running_pid"
running_pid=""
echo "web process smoke passed"
