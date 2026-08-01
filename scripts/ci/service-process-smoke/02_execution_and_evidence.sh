# Schema ownership is explicit: the migration runner must finish its startup
# migration/recovery gate before traffic-serving processes initialize their
# own persistent adapters.
migration_index=4
start_service "$migration_index"
migration_bootstrap_ready=false
for _ in $(seq 1 100); do
  if curl -fsS "http://127.0.0.1:${ports[$migration_index]}/health/ready" >/dev/null; then
    migration_bootstrap_ready=true
    break
  fi
  sleep 0.05
done
if [[ "$migration_bootstrap_ready" != true ]]; then
  cat "$temporary_directory/${services[$migration_index]}.log" >&2
  exit 1
fi

for index in "${!services[@]}"; do
  if [[ "$index" == "$migration_index" ]]; then
    continue
  fi
  start_service "$index"
done

for index in "${!services[@]}"; do
  service="${services[$index]}"
  port="${ports[$index]}"
  ready_document="$temporary_directory/$service-ready.json"
  live_document="$temporary_directory/$service-live.json"
  ready=false
  for _ in $(seq 1 100); do
    if curl -fsS "http://127.0.0.1:$port/health/ready" -o "$ready_document"; then
      ready=true
      break
    fi
    sleep 0.05
  done
  if [[ "$ready" != true ]]; then
    cat "$temporary_directory/$service.log" >&2
    exit 1
  fi
  curl -fsS "http://127.0.0.1:$port/health/live" -o "$live_document"

  python3 - "$port" <<'PY'
import socket
import sys

with socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=2) as connection:
    connection.shutdown(socket.SHUT_WR)
PY

  survived_eof=false
  for _ in $(seq 1 40); do
    if curl -fsS "http://127.0.0.1:$port/health/live" -o "$live_document"; then
      survived_eof=true
      break
    fi
    sleep 0.05
  done
  if [[ "$survived_eof" != true ]]; then
    cat "$temporary_directory/$service.log" >&2
    exit 1
  fi

  python3 - "$service" "${component_checks[$index]}" "$ready_document" "$live_document" <<'PY'
import json
import sys
from pathlib import Path

service, role_check, ready_path, live_path = sys.argv[1:]
ready = json.loads(Path(ready_path).read_text(encoding="utf-8"))
live = json.loads(Path(live_path).read_text(encoding="utf-8"))
assert ready["service"] == service
assert ready["status"] == "ready"
assert ready["state"] == "ready"
assert ready["version"]
checks = {check["name"]: check for check in ready["checks"]}
assert {"configuration", "event_registry", "listener", role_check} <= checks.keys()
assert all(check["status"] == "pass" and check["detail"] for check in checks.values())
assert live == {
    "service": service,
    "state": "ready",
    "status": "live",
    "version": ready["version"],
}
PY

  status="$({ curl -sS -o "$temporary_directory/$service-not-found.json" -w '%{http_code}' \
    "http://127.0.0.1:$port/not-a-health-route"; } 2>/dev/null)"
  test "$status" = 404
  grep -q '"error":"NOT_FOUND"' "$temporary_directory/$service-not-found.json"
done

web_pid=""
node "$root/apps/web/scripts/serve.mjs" --root dist --port 18105 \
  >"$temporary_directory/web.log" 2>&1 &
web_pid="$!"
pids+=("$web_pid")
pid_labels+=("web")
web_ready=false
for _ in $(seq 1 100); do
  if curl -fsS "http://127.0.0.1:18105/" -o "$temporary_directory/web.html"; then
    web_ready=true
    break
  fi
  sleep 0.05
done
if [[ "$web_ready" != true ]]; then
  cat "$temporary_directory/web.log" >&2
  exit 1
fi
grep -q '<title>雾港调查台</title>' "$temporary_directory/web.html"
curl -fsS "http://127.0.0.1:18105/config.json" -o "$temporary_directory/web-config.json"
python3 - "$temporary_directory/web-config.json" <<'PY'
import json
import sys
from pathlib import Path
from urllib.parse import urlparse

configuration = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
services = configuration["services"]
assert len(services) == 5
urls = [urlparse(service["url"]) for service in services]
assert all(url.scheme == "http" and url.hostname == "127.0.0.1" for url in urls)
assert {url.port for url in urls} == {8080, 8081, 8082, 8083, 8084}
PY

for pid in "${pids[@]}"; do
  kill -TERM "$pid"
done
shutdown_failed=false
for index in "${!pids[@]}"; do
  if wait "${pids[$index]}"; then
    continue
  else
    status="$?"
  fi
  label="${pid_labels[$index]}"
  printf 'service process smoke shutdown failed service=%s status=%s\n' \
    "$label" "$status" >&2
  cat "$temporary_directory/$label.log" >&2
  shutdown_failed=true
done
[[ "$shutdown_failed" == false ]]
pids=()
pid_labels=()

printf 'service process smoke: 5 services and web passed\n'
