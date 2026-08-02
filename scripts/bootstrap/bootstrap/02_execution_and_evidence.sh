if [[ ! -f "$tutorial_file" ]]; then
  tutorial_suffix="${project//-/_}"
  tutorial_created_at="$(python3 - <<'PY'
import time
print(time.time_ns() // 1_000_000)
PY
)"
  {
    printf 'TUTORIAL_CAMPAIGN_ID=%q\n' "tutorial_$tutorial_suffix"
    printf 'TUTORIAL_AUTHORITY_CONTRACT_ID=%q\n' "tutorial_authority_$tutorial_suffix"
    printf 'TUTORIAL_ROOM_ID=%q\n' "tutorial_room_$tutorial_suffix"
    printf 'TUTORIAL_CREATED_AT_UNIX_MS=%q\n' "$tutorial_created_at"
    printf 'TUTORIAL_AI_PROVIDER_SNAPSHOT=%q\n' "$project-provider"
    printf 'TUTORIAL_MODEL_ROUTE_SNAPSHOT=%q\n' "$project-provider-route-v1"
  } | atomic_text "$tutorial_file" 0600
fi
# shellcheck disable=SC1090
source "$tutorial_file"

overlay="$runtime/compose.bootstrap.yml"
if ! step_done compose_config; then
  {
    printf 'services:\n'
    printf '  agent-worker:\n'
    printf '    profiles: [staged-worker]\n'
    printf '    environment:\n'
    printf '      TRPG_LOCAL_MODEL_CERTIFICATION_REQUEST_DIRECTORY: /var/lib/trpg/model-certification-requests\n'
    printf '      TRPG_MODEL_PROVIDER_RUNTIME_SHA256: %s\n' "$provider_runtime_sha256"
    printf '    volumes: ["model_certification_requests:/var/lib/trpg/model-certification-requests:ro"]\n'
    printf '  local-model-certifier:\n'
    printf '    environment:\n'
    printf '      TRPG_LOCAL_MODEL_CERTIFICATION_REQUEST_DIRECTORY: /var/lib/trpg/model-certification-requests\n'
    printf '      TRPG_LOCAL_MODEL_CERTIFICATION_STATUS_DIRECTORY: /var/lib/trpg/model-certification-status\n'
    printf '      TRPG_MODEL_PROVIDER_RUNTIME_SHA256: %s\n' "$provider_runtime_sha256"
    printf '    volumes: ["model_certification_requests:/var/lib/trpg/model-certification-requests:ro", "model_certification_status:/var/lib/trpg/model-certification-status"]\n'
    printf '  admin:\n'
    printf '    volumes: ["model_certification_requests:/var/lib/trpg/model-certification-requests", "model_certification_status:/var/lib/trpg/model-certification-status"]\n'
    printf '    depends_on:\n'
    printf '      export-volume-init:\n'
    printf '        condition: service_completed_successfully\n'
    printf '  minio-init:\n'
    printf '    profiles: [staged-worker]\n'
    printf '  export-volume-init:\n'
    printf '    image: coc-ai-trpg/runtime:${TRPG_IMAGE_TAG:-local}\n'
    printf '    entrypoint: ["/bin/sh", "-ec"]\n'
    printf '    command: ["install -d -o trpg -g trpg -m 0700 /var/lib/trpg/exports && install -d -o trpg -g trpg -m 0700 /var/lib/trpg/local-model-certification && install -d -o trpg -g trpg -m 0700 /var/lib/trpg/model-certification-status"]\n'
    printf '    restart: "no"\n'
    printf '    read_only: true\n'
    printf '    network_mode: none\n'
    printf '    security_opt: ["no-new-privileges:true"]\n'
    printf '    volumes: ["export_artifacts:/var/lib/trpg/exports", "agent_state:/var/lib/trpg", "model_certification_status:/var/lib/trpg/model-certification-status"]\n'
    printf '  api:\n'
    printf '    depends_on:\n'
    printf '      export-volume-init:\n'
    printf '        condition: service_completed_successfully\n'
    printf 'secrets:\n'
    for name in "${secret_names[@]}"; do
      [[ -s "$secrets/$name" && ! -L "$secrets/$name" ]] || { printf 'bootstrap error=SECRET_MISSING name=%s\n' "$name" >&2; exit 4; }
      printf '  %s:\n    external: false\n    file: %s/%s\n' "$name" "$secrets" "$name"
    done
    printf 'volumes:\n'
    printf '  model_certification_requests:\n'
    printf '  model_certification_status:\n'
  } | atomic_text "$overlay" 0600
  commit_step compose_config
fi
export TRPG_CANONICAL_HMAC_KEY_ID="$project-canonical-v1"
export TRPG_PAYLOAD_ENCRYPTION_KEY_ID="$project-payload-v1"
export TRPG_AUDIT_HMAC_KEY_ID="$project-audit-v1"
export TRPG_LOCAL_MODEL_CERTIFICATION_HMAC_KEY_ID="$project-local-model-certification-v1"
export TRPG_OBJECT_STORAGE_BUCKET="trpg-$project"
export TRPG_MODEL_PROVIDER_TYPE="$runtime_provider_type"
export TRPG_MODEL_PROVIDER_ID="$project-provider"
export TRPG_MODEL_ID="$provider_model"
export TRPG_MODEL_ARTIFACT_SHA256="${provider_sha256,,}"
export TRPG_MODEL_PROVIDER_BASE_URL="$provider_url"
export TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID="$project-provider-route-v1"
compose=(docker compose --project-name "$project" -f "$root/compose.yml")
[[ -z "$extra_compose_file" ]] || compose+=(-f "$extra_compose_file")
compose+=(-f "$overlay")
worker_compose=("${compose[@]}" --profile staged-worker)
certifier_compose=("${compose[@]}" --profile staged-worker --profile local-model-certifier)
"${compose[@]}" config --quiet
step_done compose_up || { "${compose[@]}" up --detach --build --wait --wait-timeout 300; commit_step compose_up; }

if ! step_done recovery_roles; then
  "${compose[@]}" exec -T postgres sh -ec \
    'export PGPASSWORD="$(cat /run/secrets/postgres_bootstrap_password)"; exec psql -X -q -v ON_ERROR_STOP=1 -U trpg_database_owner -d postgres' <<'SQL'
SELECT 'CREATE ROLE trpg_backup_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS'
 WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_backup_login') \gexec
SELECT format('ALTER ROLE trpg_backup_login PASSWORD %L', btrim(pg_read_file('/run/secrets/postgres_backup_password'))) \gexec
SELECT 'CREATE ROLE trpg_restore_login LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS'
 WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_restore_login') \gexec
SELECT format('ALTER ROLE trpg_restore_login PASSWORD %L', btrim(pg_read_file('/run/secrets/postgres_restore_password'))) \gexec
SELECT 'CREATE DATABASE coc_ai_trpg_restore OWNER trpg_restore_login'
 WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'coc_ai_trpg_restore') \gexec
REVOKE ALL ON DATABASE coc_ai_trpg_restore FROM PUBLIC;
GRANT CONNECT ON DATABASE coc_ai_trpg TO trpg_backup_login;
GRANT CONNECT ON DATABASE coc_ai_trpg_restore TO trpg_restore_login;
\connect coc_ai_trpg_restore
CREATE EXTENSION IF NOT EXISTS vector;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO trpg_restore_login;
\connect coc_ai_trpg
GRANT USAGE ON SCHEMA public, core_domain TO trpg_backup_login;
GRANT SELECT ON ALL TABLES IN SCHEMA public, core_domain TO trpg_backup_login;
GRANT SELECT ON ALL SEQUENCES IN SCHEMA public, core_domain TO trpg_backup_login;
ALTER DEFAULT PRIVILEGES FOR ROLE trpg_database_owner IN SCHEMA public GRANT SELECT ON TABLES TO trpg_backup_login;
ALTER DEFAULT PRIVILEGES FOR ROLE trpg_database_owner IN SCHEMA core_domain GRANT SELECT ON TABLES TO trpg_backup_login;
SQL
  commit_step recovery_roles
fi

# shellcheck disable=SC1090
source "$credentials/initial-accounts.env"
api_base='https://127.0.0.1:8443/admin/v1'
product_api_base='https://127.0.0.1:8443/api'
api_call() {
  local method="$1" path="$2" headers="$3" body="$4" output="$5"
  local -a arguments=(--silent --show-error --cacert "$runtime/ca.crt" --request "$method" --header "@$headers" --output "$output" --write-out '%{http_code}')
  [[ -z "$body" ]] || arguments+=(--header 'Content-Type: application/json' --data-binary "@$body")
  curl "${arguments[@]}" "$api_base/$path"
}
product_api_call() {
  local method="$1" path="$2" headers="$3" body="$4" output="$5"
  local -a arguments=(--silent --show-error --cacert "$runtime/ca.crt" --request "$method" --header "@$headers" --output "$output" --write-out '%{http_code}')
  [[ -z "$body" ]] || arguments+=(--header 'Content-Type: application/json' --data-binary "@$body")
  curl "${arguments[@]}" "$product_api_base/$path"
}
json_value() { python3 - "$1" "$2" <<'PY'
import json, sys
value = json.load(open(sys.argv[1], encoding="utf-8"))
for part in sys.argv[2].split("."):
    value = value[part]
print(str(value).lower() if isinstance(value, bool) else value)
PY
}
response_error() { python3 - "$1" <<'PY'
import json, sys
try:
    value = json.load(open(sys.argv[1], encoding="utf-8"))
    print(value.get("error", value.get("code", "UNKNOWN_RESPONSE")))
except (OSError, ValueError, AttributeError):
    print("INVALID_RESPONSE")
PY
}
login_body="$scratch/login.json" business_login_body="$scratch/business-login.json"
complete_body="$scratch/complete.json"
python3 - "$credentials/initial-accounts.env" "$login_body" "$business_login_body" "$complete_body" <<'PY'
import json, shlex, sys
values = {}
for line in open(sys.argv[1], encoding="utf-8"):
    key, value = line.rstrip("\n").split("=", 1)
    values[key] = shlex.split(value)[0]
json.dump({"login": values["ADMIN_LOGIN"], "password": values["ADMIN_PASSWORD"]}, open(sys.argv[2], "w"))
json.dump({"login": values["BUSINESS_LOGIN"], "password": values["BUSINESS_PASSWORD"]}, open(sys.argv[3], "w"))
json.dump({"administrator": {"user_id": values["ADMIN_USER_ID"], "login": values["ADMIN_LOGIN"], "password": values["ADMIN_PASSWORD"]},
           "business_account": {"user_id": values["BUSINESS_USER_ID"], "login": values["BUSINESS_LOGIN"], "password": values["BUSINESS_PASSWORD"]}}, open(sys.argv[4], "w"))
PY
owner_headers="$scratch/owner.headers" business_headers="$scratch/business.headers"
response="$scratch/response.json"
owner_login() {
  printf 'Accept: application/json\n' >"$scratch/public.headers"
  [[ "$(api_call POST sessions "$scratch/public.headers" "$login_body" "$response")" == 200 ]] || return 1
  printf 'Authorization: Bearer %s\nAccept: application/json\n' "$(json_value "$response" access_token)" >"$owner_headers"
}
business_login() {
  printf 'Accept: application/json\n' >"$scratch/public.headers"
  [[ "$(product_api_call POST auth/login "$scratch/public.headers" "$business_login_body" "$response")" == 200 ]] || return 1
  printf 'Authorization: Bearer %s\nAccept: application/json\n' "$(json_value "$response" access_token)" >"$business_headers"
}
if ! step_done admin_bootstrap; then
  if ! owner_login; then
    printf 'Authorization: Bearer %s\nAccept: application/json\n' "$(<"$secrets/admin_bootstrap_token")" >"$scratch/bootstrap.headers"
    [[ "$(api_call GET bootstrap/status "$scratch/bootstrap.headers" '' "$response")" == 200 ]] || { printf 'bootstrap error=BOOTSTRAP_STATUS_FAILED\n' >&2; exit 5; }
    version="$(json_value "$response" state_version)"
    {
      cat "$scratch/bootstrap.headers"
      printf 'Idempotency-Key: %s-bootstrap\nX-Expected-Version: %s\nX-Correlation-Id: %s-bootstrap\nX-Causation-Id: %s-compose\n' "$project" "$version" "$project" "$project"
    } >"$scratch/mutation.headers"
    [[ "$(api_call POST bootstrap/complete "$scratch/mutation.headers" "$complete_body" "$response")" == 201 ]] || { printf 'bootstrap error=ADMIN_BOOTSTRAP_FAILED\n' >&2; exit 5; }
    owner_login || { printf 'bootstrap error=ADMIN_LOGIN_FAILED\n' >&2; exit 5; }
  fi
  commit_step admin_bootstrap
else
  owner_login || { printf 'bootstrap error=ADMIN_LOGIN_FAILED\n' >&2; exit 5; }
fi

mutation_headers() {
  local version="$1" key="$2"
  { cat "$owner_headers"; printf 'Idempotency-Key: %s\nX-Expected-Version: %s\nX-Correlation-Id: %s\nX-Causation-Id: %s-bootstrap\n' "$key" "$version" "$key" "$project"; } >"$scratch/mutation.headers"
}
if ! step_done provider_configure; then
  [[ "$(api_call GET bootstrap/status "$owner_headers" '' "$response")" == 200 ]]
  if [[ "$(json_value "$response" provider_configured)" != true ]]; then
    version="$(json_value "$response" state_version)"
    python3 - "$provider_type" "$provider_url" "$provider_model" "${provider_sha256,,}" "$scratch/provider.json" <<'PY'
import json, sys
json.dump({"provider_type": sys.argv[1], "base_url": sys.argv[2], "model_id": sys.argv[3],
           "model_artifact_sha256": sys.argv[4], "credential_secret_id": "provider_credential",
           "credential_secret_version": 1}, open(sys.argv[5], "w"))
PY
    mutation_headers "$version" "$project-provider-configure"
    [[ "$(api_call PUT providers/configuration "$scratch/mutation.headers" "$scratch/provider.json" "$response")" == 200 ]] || { printf 'bootstrap error=PROVIDER_CONFIGURATION_FAILED\n' >&2; exit 6; }
  fi
  commit_step provider_configure
fi
if ! step_done provider_probe; then
  [[ "$(api_call GET bootstrap/status "$owner_headers" '' "$response")" == 200 ]]
  mutation_headers "$(json_value "$response" state_version)" "$project-provider-probe"
  [[ "$(api_call POST providers/probe "$scratch/mutation.headers" '' "$response")" == 200 ]] || { printf 'bootstrap error=PROVIDER_PROBE_FAILED\n' >&2; exit 6; }
  commit_step provider_probe
fi
certification_request_id="$project-model-certification"
certification_state_container='/var/lib/trpg/local-model-certification'
certification_result_container="$certification_state_container/$certification_request_id.result.json"
certification_certificate_container="$certification_state_container/certificate.json"
certification_registry_container="$certification_state_container/registry.jsonl"
certification_status_container="/var/lib/trpg/model-certification-status/$certification_request_id.status.json"
if ! step_done model_certification_request; then
  if [[ "$runtime_provider_type" == cloud ]]; then
    printf 'bootstrap step=model_certification_request result=NOT_REQUIRED provider_type=cloud\n'
  else
    [[ "$(api_call GET bootstrap/status "$owner_headers" '' "$response")" == 200 ]]
    mutation_headers "$(json_value "$response" state_version)" "$certification_request_id"
    python3 - "$certification_request_id" "$provider_model" "${provider_sha256,,}" "$scratch/certification.json" <<'PY'
import json, sys
json.dump({"request_id": sys.argv[1], "model_id": sys.argv[2],
           "model_artifact_sha256": sys.argv[3]}, open(sys.argv[4], "w"))
PY
    [[ "$(api_call POST models/certification-requests "$scratch/mutation.headers" "$scratch/certification.json" "$response")" == 200 ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_REQUEST_FAILED\n' >&2; exit 6; }
    [[ "$(json_value "$response" result)" == CERTIFICATION_REQUESTED ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_REQUEST_NOT_PERSISTED\n' >&2; exit 6; }
    [[ "$(json_value "$response" artifact_reference)" == "$certification_status_container" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_STATUS_REFERENCE_INVALID\n' >&2; exit 6; }
  fi
  commit_step model_certification_request
fi
if ! step_done model_certification; then
  if [[ "$runtime_provider_type" == cloud ]]; then
    printf 'bootstrap step=model_certification result=NOT_REQUIRED provider_type=cloud\n'
  else
    "${certifier_compose[@]}" up --detach --no-build local-model-certifier
    certification_deadline=$((SECONDS + 300))
    certification_status_file="$scratch/certification-status.json"
    while true; do
      if "${certifier_compose[@]}" exec -T local-model-certifier /bin/cat \
        "$certification_status_container" >"$certification_status_file" 2>/dev/null; then
        [[ "$(json_value "$certification_status_file" schema_version)" == 1 ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_STATUS_SCHEMA_INVALID\n' >&2; exit 6; }
        [[ "$(json_value "$certification_status_file" request_id)" == "$certification_request_id" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_STATUS_REQUEST_MISMATCH\n' >&2; exit 6; }
        [[ "$(json_value "$certification_status_file" result_reference)" == "$certification_result_container" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_RESULT_REFERENCE_INVALID\n' >&2; exit 6; }
        certification_state="$(json_value "$certification_status_file" state)"
        case "$certification_state" in
          succeeded|failed) break ;;
          claimed) ;;
          *) printf 'bootstrap error=MODEL_CERTIFICATION_STATUS_INVALID state=%s\n' "$certification_state" >&2; exit 6 ;;
        esac
      fi
      ((SECONDS < certification_deadline)) || { printf 'bootstrap error=MODEL_CERTIFICATION_TIMEOUT\n' >&2; exit 6; }
      "${certifier_compose[@]}" ps --status running --services | grep -Fx local-model-certifier >/dev/null || { printf 'bootstrap error=MODEL_CERTIFIER_NOT_RUNNING\n' >&2; exit 6; }
      sleep 0.25
    done
    if ! "${certifier_compose[@]}" exec -T local-model-certifier /bin/cat \
      "$certification_result_container" >"$scratch/certification-result.json"; then
      printf 'bootstrap error=MODEL_CERTIFICATION_RESULT_UNAVAILABLE\n' >&2
      exit 6
    fi
    if [[ "$certification_state" == failed ]]; then
      certification_error="$(json_value "$certification_status_file" error_code)"
      [[ "$certification_error" == "$(json_value "$scratch/certification-result.json" error_code)" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_FAILURE_STATUS_MISMATCH\n' >&2; exit 6; }
      printf 'bootstrap error=MODEL_CERTIFICATION_TERMINAL_FAILURE detail=%s\n' "$certification_error" >&2
      exit 6
    fi
    [[ "$(json_value "$scratch/certification-result.json" state)" == succeeded ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_RESULT_NOT_SUCCESSFUL\n' >&2; exit 6; }
    [[ "$(json_value "$scratch/certification-result.json" request_id)" == "$certification_request_id" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_RESULT_REQUEST_MISMATCH\n' >&2; exit 6; }
    [[ "$(json_value "$scratch/certification-result.json" model_id)" == "$provider_model" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_RESULT_MODEL_MISMATCH\n' >&2; exit 6; }
    [[ "$(json_value "$scratch/certification-result.json" model_artifact_sha256)" == "${provider_sha256,,}" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_RESULT_ARTIFACT_MISMATCH\n' >&2; exit 6; }
    [[ "$(json_value "$scratch/certification-result.json" certificate_path)" == "$certification_certificate_container" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_RESULT_CERTIFICATE_PATH_INVALID\n' >&2; exit 6; }
    certification_evidence_container="$(json_value "$scratch/certification-result.json" evidence_path)"
    [[ "$certification_evidence_container" =~ ^/var/lib/trpg/local-model-certification/[0-9a-f]{64}\.evidence\.json$ ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_RESULT_EVIDENCE_PATH_INVALID\n' >&2; exit 6; }
    [[ "$(json_value "$certification_status_file" certificate_reference)" == "$certification_certificate_container" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_STATUS_CERTIFICATE_MISMATCH\n' >&2; exit 6; }
    "${certifier_compose[@]}" exec -T local-model-certifier /bin/cat \
      "$certification_certificate_container" >"$scratch/certificate.json"
    "${certifier_compose[@]}" exec -T local-model-certifier /bin/cat \
      "$certification_evidence_container" >"$scratch/certification-evidence.json"
    "${certifier_compose[@]}" exec -T local-model-certifier /bin/cat \
      "$certification_registry_container" >"$scratch/certification-registry.jsonl"
    [[ -s "$scratch/certification-registry.jsonl" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_REGISTRY_EMPTY\n' >&2; exit 6; }
    certification_evidence_sha256="sha256:$(sha256sum "$scratch/certification-evidence.json" | awk '{print $1}')"
    [[ "$certification_evidence_sha256" == "$(json_value "$scratch/certification-result.json" evidence_sha256)" ]] || { printf 'bootstrap error=MODEL_CERTIFICATION_EVIDENCE_HASH_MISMATCH\n' >&2; exit 6; }
    python3 - "$scratch/certification-result.json" "$scratch/certificate.json" <<'PY'
import json, sys
result = json.load(open(sys.argv[1], encoding="utf-8"))
certificate = json.load(open(sys.argv[2], encoding="utf-8"))
if certificate.get("certificate_id") != result.get("certificate_id"):
    raise SystemExit("certificate identity mismatch")
if certificate.get("model_id") != result.get("model_id"):
    raise SystemExit("certificate model mismatch")
if certificate.get("model_artifact_sha256") != result.get("model_artifact_sha256"):
    raise SystemExit("certificate artifact mismatch")
if certificate.get("level") != "Level4":
    raise SystemExit("certificate level mismatch")
binding = certificate.get("certification_binding", {})
if binding.get("provider_id") != result.get("provider_id"):
    raise SystemExit("certificate provider mismatch")
if binding.get("provider_runtime_sha256") != result.get("provider_runtime_sha256"):
    raise SystemExit("certificate runtime mismatch")
if binding.get("evidence_sha256") != result.get("evidence_sha256"):
    raise SystemExit("certificate evidence mismatch")
PY
  fi
  commit_step model_certification
fi
if ! step_done agent_worker_ready; then
  "${worker_compose[@]}" up --detach --wait --wait-timeout 300 agent-worker
  commit_step agent_worker_ready
fi
if ! step_done tutorial_authority; then
  [[ "$(api_call GET bootstrap/status "$owner_headers" '' "$response")" == 200 ]]
  mutation_headers "$(json_value "$response" state_version)" "$project-tutorial-authority"
  python3 - "$TUTORIAL_CAMPAIGN_ID" "$TUTORIAL_AUTHORITY_CONTRACT_ID" \
    "$TUTORIAL_CREATED_AT_UNIX_MS" "$TUTORIAL_AI_PROVIDER_SNAPSHOT" \
    "$TUTORIAL_MODEL_ROUTE_SNAPSHOT" "$scratch/tutorial-authority.json" <<'PY'
import json, sys
json.dump({"campaign_id": sys.argv[1], "contract_id": sys.argv[2],
           "created_at_unix_ms": int(sys.argv[3]), "ai_provider_snapshot": sys.argv[4],
           "model_route_snapshot": sys.argv[5]}, open(sys.argv[6], "w"))
PY
  tutorial_status="$(api_call POST bootstrap/tutorial-authority "$scratch/mutation.headers" "$scratch/tutorial-authority.json" "$response")"
  [[ "$tutorial_status" == 200 || "$tutorial_status" == 201 ]] || { printf 'bootstrap error=TUTORIAL_AUTHORITY_FAILED status=%s detail=%s\n' "$tutorial_status" "$(response_error "$response")" >&2; exit 7; }
  commit_step tutorial_authority
fi
if ! step_done coc7_ruleset; then
  business_login || { printf 'bootstrap error=BUSINESS_LOGIN_FAILED\n' >&2; exit 7; }
  python3 - "$project" "$BUSINESS_USER_ID" "$TUTORIAL_CAMPAIGN_ID" \
    "$TUTORIAL_AUTHORITY_CONTRACT_ID" "$TUTORIAL_ROOM_ID" \
    "$TUTORIAL_CREATED_AT_UNIX_MS" "$TUTORIAL_AI_PROVIDER_SNAPSHOT" \
    "$TUTORIAL_MODEL_ROUTE_SNAPSHOT" "$scratch/tutorial-campaign.json" <<'PY'
import json, sys
project, owner, campaign, contract, room, created, provider, route, output = sys.argv[1:]
command = {"command_id": project + "-tutorial-campaign-create",
           "idempotency_key": project + "-tutorial-campaign-create", "expected_version": 0,
           "correlation_id": project + "-tutorial-bootstrap",
           "causation_id": project + "-tutorial-authority",
           "trace_id": project + "-tutorial-campaign-trace"}
authority = {"contract_id": contract, "authority_mode": "HUMAN_KP",
             "authority_owner": owner, "ruleset_version": "coc7_rules_1",
             "house_rules_version": "coc7_house_rules_none_1",
             "scenario_version": "tutorial_mist_archive_0_1_0",
             "prompt_version": "tutorial_prompt_1", "agent_pack_version": "tutorial_agent_pack_1",
             "tool_schema_version": "tutorial_tool_schema_1",
             "safety_profile_version": "tutorial_safety_profile_1",
             "ai_provider_snapshot": provider, "model_route_snapshot": route,
             "character_sheet_template_version": "coc7_investigator_1"}
json.dump({"command": command, "campaign_id": campaign, "owner_user_id": owner,
           "title": "COC7 Tutorial: 灰港档案室", "room_id": room,
           "room_name": "Tutorial Room", "created_at_unix_ms": int(created),
           "authority": authority}, open(output, "w"))
PY
  tutorial_status="$(product_api_call POST api/v1/campaigns "$business_headers" "$scratch/tutorial-campaign.json" "$response")"
  [[ "$tutorial_status" == 200 || "$tutorial_status" == 201 ]] || { printf 'bootstrap error=COC7_RULESET_INITIALIZATION_FAILED status=%s detail=%s\n' "$tutorial_status" "$(response_error "$response")" >&2; exit 7; }
  commit_step coc7_ruleset
fi
if ! step_done tutorial_scenario; then
  business_login || { printf 'bootstrap error=BUSINESS_LOGIN_FAILED\n' >&2; exit 7; }
  python3 - "$project" "$TUTORIAL_CAMPAIGN_ID" \
    "$root/fixtures/scenarios/tutorial_mist_archive.scenario.json" \
    "$scratch/tutorial-scenario.json" <<'PY'
import hashlib, json, sys
project, campaign, source, output = sys.argv[1:]
raw = open(source, encoding="utf-8").read().strip()
document = json.loads(raw)
canonical = json.dumps(document, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
digest = "sha256:" + hashlib.sha256(canonical.encode()).hexdigest()
if raw != canonical or digest != "sha256:7547627b443af88f925e341481f7542df238f30cc6918ece00a445033e10d4cc":
    raise SystemExit("tutorial scenario fixture is not canonical")
command = {"command_id": project + "-tutorial-scenario-import",
           "idempotency_key": project + "-tutorial-scenario-import", "expected_version": 0,
           "correlation_id": project + "-tutorial-bootstrap",
           "causation_id": project + "-tutorial-campaign-create",
           "trace_id": project + "-tutorial-scenario-trace"}
json.dump({"command": command, "campaign_id": campaign,
           "scenario_id": "tutorial_mist_archive", "ruleset_id": "coc7",
           "format_version": "0.1.0", "content_hash": digest,
           "document_json": canonical}, open(output, "w"))
PY
  tutorial_status="$(product_api_call POST "api/v1/campaigns/$TUTORIAL_CAMPAIGN_ID/scenarios/import" "$business_headers" "$scratch/tutorial-scenario.json" "$response")"
  [[ "$tutorial_status" == 200 || "$tutorial_status" == 201 ]] || { printf 'bootstrap error=TUTORIAL_SCENARIO_IMPORT_FAILED status=%s detail=%s\n' "$tutorial_status" "$(response_error "$response")" >&2; exit 7; }
  commit_step tutorial_scenario
fi
if ! step_done self_check; then
  [[ "$(api_call GET diagnostics "$owner_headers" '' "$response")" == 200 ]]
  [[ "$(api_call GET audit "$owner_headers" '' "$response")" == 200 ]]
  business_login || { printf 'bootstrap error=BUSINESS_LOGIN_FAILED\n' >&2; exit 7; }
  [[ "$(product_api_call GET "api/v1/campaigns/$TUTORIAL_CAMPAIGN_ID" "$business_headers" '' "$response")" == 200 ]]
  [[ "$(json_value "$response" campaign_id)" == "$TUTORIAL_CAMPAIGN_ID" ]]
  [[ "$(json_value "$response" owner_user_id)" == "$BUSINESS_USER_ID" ]]
  [[ "$(json_value "$response" authority_contract_id)" == "$TUTORIAL_AUTHORITY_CONTRACT_ID" ]]
  curl --fail --silent --show-error --cacert "$runtime/ca.crt" https://127.0.0.1:8443/ >/dev/null
  commit_step self_check
fi
printf 'bootstrap complete state=%s credentials=%s endpoint=https://127.0.0.1:8443\n' \
  "$journal" "$credentials/initial-accounts.env"
