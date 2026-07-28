# Responsibility-focused phase sourced by production-security-smoke.sh.
plaintext_proxy_status="$(
  curl --silent --output /dev/null \
    --write-out '%{http_code} %{redirect_url}' http://localhost:8080/ || true
)"
if [[ "$plaintext_proxy_status" != 308\ https://* ]]; then
  printf 'reverse proxy plaintext endpoint did not enforce the HTTPS redirect: %s\n' \
    "$plaintext_proxy_status" >&2
  exit 1
fi
if curl --fail --silent http://localhost:8443/ >/dev/null 2>&1; then
  printf 'reverse proxy TLS port accepted plaintext HTTP\n' >&2
  exit 1
fi
for runtime_path in \
  / \
  /api/health/ready \
  /realtime/health/ready \
  /admin/health/ready; do
  curl --fail --silent --show-error \
    --cacert "$runtime_directory/ca.crt" \
    "https://localhost:8443$runtime_path" >/dev/null
done

swarm_state="$(docker info --format '{{.Swarm.LocalNodeState}}')"
if [[ "$swarm_state" == inactive ]]; then
  docker swarm init --advertise-addr 127.0.0.1 >/dev/null
  initialized_swarm=true
elif [[ "$swarm_state" != active ]]; then
  printf 'Docker Swarm is neither inactive nor active: %s\n' "$swarm_state" >&2
  exit 1
fi
docker secret create "$swarm_secret_v1" "$runtime_directory/minio_server_v1.crt" >/dev/null
docker secret create "$swarm_secret_v2" "$runtime_directory/minio_server_v2.crt" >/dev/null
docker service create \
  --name "$swarm_service" \
  --constraint node.role==manager \
  --restart-condition none \
  --secret "source=$swarm_secret_v1,target=server.crt" \
  --entrypoint /bin/sh \
  nginx:1.27-alpine@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10 \
  -c 'test -s /run/secrets/server.crt; while :; do sleep 30; done' >/dev/null

canary_container="$(wait_for_task_container "$swarm_service")"
expected_v1="$(sha256sum "$runtime_directory/minio_server_v1.crt" | awk '{print $1}')"
mounted_v1="$(docker exec "$canary_container" sha256sum /run/secrets/server.crt | awk '{print $1}')"
if [[ "$mounted_v1" != "$expected_v1" ]]; then
  printf 'Docker-managed external secret v1 was not mounted byte-for-byte\n' >&2
  exit 1
fi

docker service update \
  --secret-rm "$swarm_secret_v1" \
  --secret-add "source=$swarm_secret_v2,target=server.crt" \
  "$swarm_service" >/dev/null
rotated_container="$(wait_for_task_container "$swarm_service" "$canary_container")"
expected_v2="$(sha256sum "$runtime_directory/minio_server_v2.crt" | awk '{print $1}')"
mounted_v2="$(docker exec "$rotated_container" sha256sum /run/secrets/server.crt | awk '{print $1}')"
if [[ "$mounted_v2" != "$expected_v2" ]]; then
  printf 'Docker-managed external secret v2 was not mounted after rotation\n' >&2
  exit 1
fi
docker secret rm "$swarm_secret_v1" >/dev/null

printf 'production security smoke: full product graph, witness least privilege, TLS, Redis/NATS mTLS, certificate rotation, and external-secret rotation passed\n'
