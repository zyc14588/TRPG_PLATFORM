#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'
umask 077

if [[ "$#" -ne 1 ]]; then
  printf 'usage: %s EVIDENCE_DIRECTORY\n' "$0" >&2
  exit 2
fi

evidence_directory="$1"
[[ "$evidence_directory" = /* && ! -L "$evidence_directory" ]] || {
  printf 'EVIDENCE_DIRECTORY must be an absolute non-symlink path\n' >&2
  exit 2
}
install -d -m 0700 "$evidence_directory"

for command_name in cargo curl ollama openssl python3 sha256sum; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'real local provider matrix requires command: %s\n' "$command_name" >&2
    exit 2
  }
done
llama_server="${TRPG_LLAMA_SERVER_BIN:-$(command -v llama-server || true)}"
[[ "$llama_server" = /* && -x "$llama_server" && ! -L "$llama_server" ]] || {
  printf 'TRPG_LLAMA_SERVER_BIN must name an absolute executable non-symlink file\n' >&2
  exit 2
}

ollama_chat_model="${TRPG_OLLAMA_CHAT_MODEL:-qwen3.6:35b}"
ollama_embedding_model="${TRPG_OLLAMA_EMBEDDING_MODEL:-qwen3-embedding:8b}"
llama_chat_model_path="${TRPG_LLAMA_CPP_CHAT_MODEL_PATH:?TRPG_LLAMA_CPP_CHAT_MODEL_PATH is required}"
llama_embedding_model_path="${TRPG_LLAMA_CPP_EMBEDDING_MODEL_PATH:?TRPG_LLAMA_CPP_EMBEDDING_MODEL_PATH is required}"
for model_path in "$llama_chat_model_path" "$llama_embedding_model_path"; do
  [[ "$model_path" = /* && -f "$model_path" && ! -L "$model_path" ]] || {
    printf 'llama.cpp model must be an absolute regular non-symlink file: %s\n' "$model_path" >&2
    exit 2
  }
done
require_cloud_provider="${TRPG_REQUIRE_REAL_CLOUD_PROVIDER:-0}"
case "$require_cloud_provider" in
  1)
    cloud_provider_url="${TRPG_REAL_CLOUD_PROVIDER_URL:?TRPG_REAL_CLOUD_PROVIDER_URL is required}"
    cloud_chat_model="${TRPG_REAL_CLOUD_CHAT_MODEL:?TRPG_REAL_CLOUD_CHAT_MODEL is required}"
    cloud_embedding_model="${TRPG_REAL_CLOUD_EMBEDDING_MODEL:?TRPG_REAL_CLOUD_EMBEDDING_MODEL is required}"
    cloud_chat_sha256="${TRPG_REAL_CLOUD_CHAT_MODEL_SHA256:?TRPG_REAL_CLOUD_CHAT_MODEL_SHA256 is required}"
    cloud_embedding_sha256="${TRPG_REAL_CLOUD_EMBEDDING_MODEL_SHA256:?TRPG_REAL_CLOUD_EMBEDDING_MODEL_SHA256 is required}"
    cloud_credential_path="${TRPG_REAL_CLOUD_CREDENTIAL_PATH:?TRPG_REAL_CLOUD_CREDENTIAL_PATH is required}"
    [[ "$cloud_provider_url" == https://* ]] || {
      printf 'TRPG_REAL_CLOUD_PROVIDER_URL must use HTTPS\n' >&2
      exit 2
    }
    for digest in "$cloud_chat_sha256" "$cloud_embedding_sha256"; do
      [[ "$digest" =~ ^sha256:[0-9a-f]{64}$ ]] || {
        printf 'real cloud model identity must be a sha256 digest\n' >&2
        exit 2
      }
    done
    [[ "$cloud_credential_path" = /* && -s "$cloud_credential_path" && ! -L "$cloud_credential_path" ]] || {
      printf 'TRPG_REAL_CLOUD_CREDENTIAL_PATH must name an absolute nonempty non-symlink file\n' >&2
      exit 2
    }
    credential_mode="$(stat -c '%a' "$cloud_credential_path")"
    if [[ ! "$credential_mode" =~ ^[0-7]{3,4}$ ]] || (( (8#$credential_mode & 077) != 0 )); then
      printf 'TRPG_REAL_CLOUD_CREDENTIAL_PATH must not grant group or other access\n' >&2
      exit 2
    fi
    ;;
  0) ;;
  *)
    printf 'TRPG_REQUIRE_REAL_CLOUD_PROVIDER must be 0 or 1\n' >&2
    exit 2
    ;;
esac

test_root="$(mktemp -d)"
pids=()
cleanup() {
  local code="$?" pid
  for pid in "${pids[@]}"; do
    kill "$pid" >/dev/null 2>&1 || true
  done
  for pid in "${pids[@]}"; do
    wait "$pid" >/dev/null 2>&1 || true
  done
  rm -rf "$test_root"
  return "$code"
}
trap cleanup EXIT

free_port() {
  python3 - <<'PY'
import socket

with socket.socket() as listener:
    listener.bind(("127.0.0.1", 0))
    print(listener.getsockname()[1])
PY
}

wait_http() {
  local url="$1" process_id="$2"
  for _ in {1..180}; do
    kill -0 "$process_id" 2>/dev/null || {
      printf 'provider process exited before readiness: %s\n' "$url" >&2
      return 1
    }
    if curl --fail --silent --show-error --max-time 2 "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  printf 'provider readiness timed out: %s\n' "$url" >&2
  return 1
}

kill_process() {
  local process_id="$1"
  kill "$process_id" >/dev/null 2>&1 || true
  wait "$process_id" >/dev/null 2>&1 || true
}

certificate_directory="$test_root/certificates"
install -d -m 0700 "$certificate_directory"
printf '%s' "$(openssl rand -hex 32)" >"$certificate_directory/token"
openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 1 \
  -subj '/CN=TRPG Real Provider Root' \
  -keyout "$certificate_directory/ca.key" \
  -out "$certificate_directory/ca.crt" >/dev/null 2>&1
openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 1 \
  -subj '/CN=TRPG Wrong Provider Root' \
  -keyout "$certificate_directory/wrong-ca.key" \
  -out "$certificate_directory/wrong-ca.crt" >/dev/null 2>&1
openssl req -newkey rsa:2048 -nodes -sha256 -subj '/CN=localhost' \
  -keyout "$certificate_directory/server.key" \
  -out "$certificate_directory/server.csr" >/dev/null 2>&1
printf '%s\n' \
  'basicConstraints=critical,CA:FALSE' \
  'keyUsage=critical,digitalSignature,keyEncipherment' \
  'extendedKeyUsage=serverAuth' \
  'subjectAltName=DNS:localhost' >"$certificate_directory/server.ext"
openssl x509 -req -sha256 -days 1 \
  -in "$certificate_directory/server.csr" \
  -CA "$certificate_directory/ca.crt" \
  -CAkey "$certificate_directory/ca.key" \
  -set_serial 2505 \
  -extfile "$certificate_directory/server.ext" \
  -out "$certificate_directory/server.crt" >/dev/null 2>&1

start_tls_proxy() {
  local listen_port="$1" upstream="$2" ready_file="$3" log_file="$4" proxy_status
  python3 - "$listen_port" "$upstream" "$certificate_directory/token" \
    "$certificate_directory/server.crt" "$certificate_directory/server.key" \
    "$ready_file" >"$log_file" 2>&1 <<'PY' &
import http.client
import http.server
import json
import ssl
import sys
import urllib.parse
from pathlib import Path

listen_port = int(sys.argv[1])
upstream = urllib.parse.urlsplit(sys.argv[2])
token = Path(sys.argv[3]).read_text(encoding="utf-8")
certificate = sys.argv[4]
private_key = sys.argv[5]
ready_file = Path(sys.argv[6])
if upstream.scheme != "http" or upstream.hostname not in {"127.0.0.1", "localhost"}:
    raise SystemExit("upstream must be loopback HTTP")

hop_headers = {
    "authorization",
    "connection",
    "content-length",
    "host",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
}

class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        self.forward()

    def do_POST(self):
        self.forward()

    def forward(self):
        if self.headers.get("Authorization") != f"Bearer {token}":
            body = b'{"error":"unauthorized"}'
            self.send_response(401)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length) if length else None
        headers = {
            key: value
            for key, value in self.headers.items()
            if key.lower() not in hop_headers
        }
        connection = http.client.HTTPConnection(
            upstream.hostname, upstream.port, timeout=310
        )
        try:
            connection.request(self.command, self.path, body=body, headers=headers)
            response = connection.getresponse()
            response_body = response.read()
            self.send_response(response.status)
            for key, value in response.getheaders():
                if key.lower() not in hop_headers:
                    self.send_header(key, value)
            self.send_header("Content-Length", str(len(response_body)))
            self.end_headers()
            self.wfile.write(response_body)
            print(f"{self.command} {self.path} -> {response.status}", flush=True)
        except Exception as error:
            response_body = json.dumps({"error": "upstream_failure"}).encode()
            self.send_response(502)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(response_body)))
            self.end_headers()
            self.wfile.write(response_body)
            print(f"upstream failure: {type(error).__name__}", flush=True)
        finally:
            connection.close()

    def log_message(self, *_args):
        return

server = http.server.ThreadingHTTPServer(("127.0.0.1", listen_port), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(certificate, private_key)
server.socket = context.wrap_socket(server.socket, server_side=True)
ready_file.write_text(str(listen_port), encoding="ascii")
server.serve_forever()
PY
  proxy_pid=$!
  pids+=("$proxy_pid")
  for _ in {1..100}; do
    [[ -s "$ready_file" ]] && return 0
    if ! kill -0 "$proxy_pid" 2>/dev/null; then
      if wait "$proxy_pid"; then
        proxy_status=0
      else
        proxy_status=$?
      fi
      printf 'TLS proxy exited before readiness for %s (status=%s, log=%s)\n' \
        "$upstream" "$proxy_status" "$log_file" >&2
      return 1
    fi
    sleep 0.1
  done
  printf 'TLS proxy did not become ready for %s\n' "$upstream" >&2
  return 1
}

run_contract() {
  local provider_type="$1" chat_url="$2" embedding_url="$3"
  local chat_model="$4" embedding_model="$5" chat_sha="$6" embedding_sha="$7"
  local runtime_sha="$8" credential_path="${9:-$certificate_directory/token}"
  TRPG_REAL_PROVIDER_TYPE="$provider_type" \
  TRPG_REAL_CHAT_PROVIDER_URL="$chat_url" \
  TRPG_REAL_EMBEDDING_PROVIDER_URL="$embedding_url" \
  TRPG_REAL_CHAT_MODEL="$chat_model" \
  TRPG_REAL_EMBEDDING_MODEL="$embedding_model" \
  TRPG_REAL_CHAT_MODEL_SHA256="$chat_sha" \
  TRPG_REAL_EMBEDDING_MODEL_SHA256="$embedding_sha" \
  TRPG_REAL_PROVIDER_CREDENTIAL_PATH="$credential_path" \
  TRPG_REAL_PROVIDER_CA_PATH="$certificate_directory/ca.crt" \
  TRPG_REAL_WRONG_PROVIDER_CA_PATH="$certificate_directory/wrong-ca.crt" \
  TRPG_REAL_WRONG_HOST_PROVIDER_URL="${chat_url/localhost/127.0.0.1}" \
  TRPG_REAL_PROVIDER_ALLOWLIST=loopback \
  TRPG_MODEL_PROVIDER_RUNTIME_SHA256="$runtime_sha" \
    cargo test -p trpg-agent-runtime \
      --test real_local_provider_contract_tests \
      authenticated_real_provider_satisfies_positive_and_negative_contracts \
      -- --ignored --exact --nocapture
}

ollama_port="$(free_port)"
ollama_proxy_port="$(free_port)"
OLLAMA_HOST="127.0.0.1:$ollama_port" \
  OLLAMA_CONTEXT_LENGTH=4096 \
  OLLAMA_NUM_PARALLEL=1 \
  ollama serve >"$evidence_directory/real-ollama-server.log" 2>&1 &
ollama_pid=$!
pids+=("$ollama_pid")
wait_http "http://127.0.0.1:$ollama_port/api/tags" "$ollama_pid"
ollama_models="$(curl --fail --silent --show-error "http://127.0.0.1:$ollama_port/api/tags")"
ollama_chat_digest="$(python3 -c 'import json,sys; p=json.load(sys.stdin); n=sys.argv[1]; print(next((m["digest"] for m in p["models"] if m["name"] == n), ""))' "$ollama_chat_model" <<<"$ollama_models")"
ollama_embedding_digest="$(python3 -c 'import json,sys; p=json.load(sys.stdin); n=sys.argv[1]; print(next((m["digest"] for m in p["models"] if m["name"] == n), ""))' "$ollama_embedding_model" <<<"$ollama_models")"
[[ "$ollama_chat_digest" =~ ^[0-9a-f]{64}$ ]] && ollama_chat_digest="sha256:$ollama_chat_digest"
[[ "$ollama_embedding_digest" =~ ^[0-9a-f]{64}$ ]] && ollama_embedding_digest="sha256:$ollama_embedding_digest"
[[ "$ollama_chat_digest" =~ ^sha256:[0-9a-f]{64}$ ]] || {
  printf 'Ollama chat model is not installed with a verifiable digest: %s\n' "$ollama_chat_model" >&2
  exit 1
}
[[ "$ollama_embedding_digest" =~ ^sha256:[0-9a-f]{64}$ ]] || {
  printf 'Ollama embedding model is not installed with a verifiable digest: %s\n' "$ollama_embedding_model" >&2
  exit 1
}
start_tls_proxy "$ollama_proxy_port" "http://127.0.0.1:$ollama_port" \
  "$test_root/ollama-proxy.ready" \
  "$evidence_directory/real-ollama-tls-proxy.log"
ollama_proxy_pid="$proxy_pid"
ollama_runtime_sha="sha256:$(sha256sum "$(command -v ollama)" | awk '{print $1}')"
run_contract ollama \
  "https://localhost:$ollama_proxy_port/" \
  "https://localhost:$ollama_proxy_port/" \
  "$ollama_chat_model" "$ollama_embedding_model" \
  "$ollama_chat_digest" "$ollama_embedding_digest" \
  "$ollama_runtime_sha"
printf 'provider_artifact provider=ollama chat_model=%s chat_sha256=%s embedding_model=%s embedding_sha256=%s runtime_sha256=%s\n' \
  "$ollama_chat_model" "$ollama_chat_digest" \
  "$ollama_embedding_model" "$ollama_embedding_digest" "$ollama_runtime_sha"
printf 'test real_local_provider::ollama_fresh_path ... ok\n'
kill_process "$ollama_proxy_pid"
kill_process "$ollama_pid"

llama_chat_port="$(free_port)"
llama_embedding_port="$(free_port)"
llama_chat_proxy_port="$(free_port)"
llama_embedding_proxy_port="$(free_port)"
"$llama_server" --host 127.0.0.1 --port "$llama_chat_port" \
  --model "$llama_chat_model_path" --alias trpg-llama-chat --jinja \
  --ctx-size 4096 --parallel 1 \
  >"$evidence_directory/real-llama-cpp-chat.log" 2>&1 &
llama_chat_pid=$!
pids+=("$llama_chat_pid")
"$llama_server" --host 127.0.0.1 --port "$llama_embedding_port" \
  --model "$llama_embedding_model_path" --alias trpg-llama-embedding \
  --embedding --pooling mean --ctx-size 4096 --parallel 1 \
  >"$evidence_directory/real-llama-cpp-embedding.log" 2>&1 &
llama_embedding_pid=$!
pids+=("$llama_embedding_pid")
wait_http "http://127.0.0.1:$llama_chat_port/health" "$llama_chat_pid"
wait_http "http://127.0.0.1:$llama_embedding_port/health" "$llama_embedding_pid"
start_tls_proxy "$llama_chat_proxy_port" "http://127.0.0.1:$llama_chat_port" \
  "$test_root/llama-chat-proxy.ready" \
  "$evidence_directory/real-llama-cpp-chat-tls-proxy.log"
llama_chat_proxy_pid="$proxy_pid"
start_tls_proxy "$llama_embedding_proxy_port" "http://127.0.0.1:$llama_embedding_port" \
  "$test_root/llama-embedding-proxy.ready" \
  "$evidence_directory/real-llama-cpp-embedding-tls-proxy.log"
llama_embedding_proxy_pid="$proxy_pid"
llama_chat_sha256="sha256:$(sha256sum "$llama_chat_model_path" | awk '{print $1}')"
llama_embedding_sha256="sha256:$(sha256sum "$llama_embedding_model_path" | awk '{print $1}')"
llama_runtime_sha256="sha256:$(sha256sum "$llama_server" | awk '{print $1}')"
run_contract llama_cpp \
  "https://localhost:$llama_chat_proxy_port/v1/" \
  "https://localhost:$llama_embedding_proxy_port/v1/" \
  trpg-llama-chat trpg-llama-embedding \
  "$llama_chat_sha256" "$llama_embedding_sha256" "$llama_runtime_sha256"
printf 'provider_artifact provider=llama_cpp chat_model=%s chat_sha256=%s embedding_model=%s embedding_sha256=%s runtime_sha256=%s\n' \
  trpg-llama-chat "$llama_chat_sha256" \
  trpg-llama-embedding "$llama_embedding_sha256" "$llama_runtime_sha256"
printf 'test real_local_provider::llama_cpp_fresh_path ... ok\n'
kill_process "$llama_chat_proxy_pid"
kill_process "$llama_embedding_proxy_pid"
kill_process "$llama_chat_pid"
kill_process "$llama_embedding_pid"

if [[ "$require_cloud_provider" == 1 ]]; then
  cloud_runtime_sha256="sha256:$(sha256sum scripts/ci/real-local-provider-matrix.sh | awk '{print $1}')"
  run_contract cloud \
    "$cloud_provider_url" "$cloud_provider_url" \
    "$cloud_chat_model" "$cloud_embedding_model" \
    "$cloud_chat_sha256" "$cloud_embedding_sha256" \
    "$cloud_runtime_sha256" "$cloud_credential_path"
  printf 'provider_artifact provider=cloud chat_model=%s chat_sha256=%s embedding_model=%s embedding_sha256=%s adapter_harness_sha256=%s\n' \
    "$cloud_chat_model" "$cloud_chat_sha256" \
    "$cloud_embedding_model" "$cloud_embedding_sha256" "$cloud_runtime_sha256"
  printf 'test real_cloud_provider::fresh_path ... ok\n'
fi

for log in "$evidence_directory"/real-*.log; do
  printf 'provider_log sha256=%s bytes=%s file=%s\n' \
    "$(sha256sum "$log" | awk '{print $1}')" \
    "$(stat -c '%s' "$log")" \
    "$(basename "$log")"
done
printf 'fresh Ollama and llama.cpp provider matrix passed\n'
