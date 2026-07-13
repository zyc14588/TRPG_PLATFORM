#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

if [[ "${TRPG_SKIP_BUILD:-0}" != "1" ]]; then
  cargo build --workspace --all-targets --release --locked
  pnpm --filter ./apps/web... build
fi

tmp_dir="$(mktemp -d)"
running_pid=""
service_pid=""
if command -v python3 >/dev/null 2>&1; then
  python=python3
elif command -v python >/dev/null 2>&1; then
  python=python
else
  echo "Python is required for health contract validation" >&2
  exit 1
fi
windows_native=0
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) windows_native=1 ;;
esac
unset TRPG_MAX_REQUESTS
cleanup() {
  if [[ "$windows_native" == "1" && -n "$service_pid" ]]; then
    "$python" -c '
import os
import signal
import sys

try:
    os.kill(int(sys.argv[1]), signal.CTRL_BREAK_EVENT)
except OSError:
    pass
' "$service_pid" >/dev/null 2>&1 || true
  fi
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
  local expected_checks="$4"
  local expected_check_status="$5"
  "$python" -c '
import json
import sys

payload = json.load(open(sys.argv[1], encoding="utf-8"))
assert payload["service"] == sys.argv[2], payload
assert payload["status"] == sys.argv[3], payload
assert isinstance(payload.get("version"), str) and payload["version"], payload
assert isinstance(payload.get("checks"), list) and payload["checks"], payload
assert {check.get("name") for check in payload["checks"]} == set(sys.argv[4].split(",")), payload
assert all(check.get("status") == sys.argv[5] for check in payload["checks"]), payload
' "$file" "$service" "$status" "$expected_checks" "$expected_check_status"
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
  local expected_checks="$3"
  local mode="${4:-ready}"
  local binary
  local base_url="http://127.0.0.1:$port"
  local instance="$service-$mode"
  local log="$tmp_dir/$instance.log"
  local live_body="$tmp_dir/$instance-live.json"
  local live_headers="$tmp_dir/$instance-live.headers"
  local ready_status="ready"
  local ready_code_expected="200"
  local ready_check_status="pass"
  local env_args=("TRPG_BIND_ADDR=127.0.0.1:$port")
  local pid_file=""
  local exit_file=""
  binary="$(binary_path "$service")"

  if [[ "$mode" == "unsafe-production" ]]; then
    ready_status="not_ready"
    ready_code_expected="503"
    ready_check_status="fail"
    env_args+=(
      "TRPG_ENVIRONMENT=production"
      "TRPG_PROVIDER=local-runtime"
      "TRPG_PROVIDER_API_KEY="
      "TRPG_PROVIDER_BASE_URL=http://127.0.0.1:11434"
      "TRPG_PROVIDER_AUTHENTICATED=false"
    )
  elif [[ "$service" == "admin-server" ]]; then
    env_args+=("TRPG_ENVIRONMENT=development")
  fi

  if [[ "$windows_native" == "1" ]]; then
    pid_file="$tmp_dir/$instance.pid"
    exit_file="$tmp_dir/$instance.exit"
    env "${env_args[@]}" "$python" -c '
import os
from pathlib import Path
import subprocess
import sys

binary, pid_file, exit_file, log_file = sys.argv[1:]
with open(log_file, "wb") as output:
    child = subprocess.Popen(
        [binary],
        env=os.environ,
        stdout=output,
        stderr=subprocess.STDOUT,
        creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
    )
    Path(pid_file).write_text(str(child.pid), encoding="utf-8")
    exit_code = child.wait()
Path(exit_file).write_text(str(exit_code), encoding="utf-8")
raise SystemExit(0 if exit_code == 0 else 1)
' "$binary" "$pid_file" "$exit_file" "$log" &
    running_pid=$!
    for _ in {1..50}; do
      [[ -s "$pid_file" ]] && break
      if ! kill -0 "$running_pid" >/dev/null 2>&1; then
        break
      fi
      sleep 0.1
    done
    if [[ ! -s "$pid_file" ]]; then
      wait "$running_pid" || true
      cat "$log" >&2 || true
      echo "$service launcher exited before recording its child process" >&2
      return 1
    fi
    service_pid="$(cat "$pid_file")"
  else
    env "${env_args[@]}" "$binary" >"$log" 2>&1 &
    running_pid=$!
    service_pid="$running_pid"
  fi

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
  assert_health_json "$live_body" "$service" "live" "listener" "pass"
  grep -qi '^Access-Control-Allow-Origin: \*' "$live_headers"
  grep -qi '^Cache-Control: no-store' "$live_headers"

  local ready_body="$tmp_dir/$instance-ready.json"
  local ready_code
  ready_code="$(curl -sS -o "$ready_body" -w '%{http_code}' "$base_url/health/ready")"
  [[ "$ready_code" == "$ready_code_expected" ]]
  assert_health_json "$ready_body" "$service" "$ready_status" "$expected_checks" "$ready_check_status"

  local missing_body="$tmp_dir/$instance-missing.json"
  local missing_code
  missing_code="$(curl -sS -o "$missing_body" -w '%{http_code}' "$base_url/not-a-health-route")"
  [[ "$missing_code" == "404" ]]
  assert_health_json "$missing_body" "$service" "not_found" "request" "fail"

  local method_body="$tmp_dir/$instance-method.json"
  local method_headers="$tmp_dir/$instance-method.headers"
  local method_code
  method_code="$(curl -sS -X POST -D "$method_headers" -o "$method_body" -w '%{http_code}' "$base_url/health/live")"
  [[ "$method_code" == "405" ]]
  assert_health_json "$method_body" "$service" "method_not_allowed" "request" "fail"
  grep -qi '^Allow: GET' "$method_headers"

  if [[ "$windows_native" == "1" ]]; then
    "$python" -c '
import os
import signal
import sys

os.kill(int(sys.argv[1]), signal.CTRL_BREAK_EVENT)
' "$service_pid"
  else
    kill -TERM "$service_pid"
  fi
  local exited=0
  for _ in {1..100}; do
    if ! kill -0 "$running_pid" >/dev/null 2>&1; then
      exited=1
      break
    fi
    sleep 0.05
  done
  if [[ "$exited" != "1" ]]; then
    cat "$log" >&2
    echo "$service did not exit after its shutdown signal" >&2
    return 1
  fi
  if ! wait "$running_pid"; then
    cat "$log" >&2
    echo "$service did not exit cleanly after its shutdown signal" >&2
    return 1
  fi
  if [[ "$windows_native" == "1" && "$(cat "$exit_file")" != "0" ]]; then
    cat "$log" >&2
    echo "$service returned a non-zero child exit code" >&2
    return 1
  fi
  grep -q "$service shutdown complete" "$log"
  running_pid=""
  service_pid=""
  printf 'service process smoke passed: %s (%s)\n' "$service" "$mode"
}

smoke_service api-server 18080 "api_contract_registry,api_adapter_boundaries"
smoke_service realtime-server 18081 "realtime_runtime_boundary"
smoke_service agent-worker 18082 "agent_provider_boundary,silent_fallback_gate"
smoke_service admin-server 18083 "admin_provider_boundary"
smoke_service migration-runner 18084 "migration_registry"
smoke_service admin-server 18085 "admin_provider_boundary" "unsafe-production"

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
