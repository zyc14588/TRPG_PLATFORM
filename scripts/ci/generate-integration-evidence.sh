#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  printf 'usage: %s REPORT_PATH\n' "$0" >&2
  exit 2
fi

report_path="$1"
environment_keys=(
  P02_DATABASE_URL
  P02_WORKER_SERVICE_DATABASE_URL
  P02_CANONICAL_SERVICE_DATABASE_URL
  P02_REDIS_URL
  AR03_WITNESS_DATABASE_URL
  P02_CANONICAL_DATABASE_URL
  P02_CANONICAL_WITNESS_DATABASE_URL
  P02_CANONICAL_ALLOW_DATABASE_RESET
  P02_CANONICAL_RESET_DATABASE
  P02_CANONICAL_WITNESS_RESET_DATABASE
  P02_EVENTING_DATABASE_URL
  P02_EVENTING_WITNESS_DATABASE_URL
  P02_EVENTING_ALLOW_DATABASE_RESET
  P02_EVENTING_RESET_DATABASE
  P02_EVENTING_WITNESS_RESET_DATABASE
  P02_WORKFLOW_DATABASE_URL
  P02_API_CANONICAL_DATABASE_URL
  P02_API_CANONICAL_WITNESS_DATABASE_URL
  P02_FORMAL_COMMIT_DATABASE_URL
  P02_FORMAL_COMMIT_WITNESS_DATABASE_URL
  P02_WITNESS_DATABASE_URL
  P02_WITNESS_APPEND_DATABASE_URL
  P02_NATS_URL
  P02_OPENFGA_ADDRESS
  P02_OPENFGA_STORE_ID
  P02_OPENFGA_MODEL_ID
  P02_OPA_ADDRESS
  P02_OPA_REVISION
  P02_TLS_DATABASE_URL
  P02_TLS_CA_CERT_PATH
  P02_PG_DUMP
  P02_PG_RESTORE
  P02_PSQL
  P02_LIBPQ_SERVICE_FILE
  P02_BACKUP_SOURCE_SERVICE
  P02_BACKUP_TARGET_SERVICE
  P02_BACKUP_SOURCE_URL
  P02_BACKUP_TARGET_URL
  P02_BACKUP_DIR
  P03_DATABASE_URL
  P03_ALLOW_DATABASE_RESET
  P03_SCHEMA_ASSERTION_PATH
  P04_DATABASE_URL
  P04_WITNESS_DATABASE_URL
  P04_ALLOW_DATABASE_RESET
  P04_ADMIN_DATABASE_URL
  P04_RECOVERY_DATABASE_URL
  P04_PG_DUMP
  P04_PG_RESTORE
  P05_DATABASE_URL
  P05_WORKER_DATABASE_URL
  P05_WITNESS_DATABASE_URL
  P05_REDIS_URL
  P05_NATS_URL
  P05_MINIO_ENDPOINT
  P05_MINIO_REGION
  P05_MINIO_BUCKET
  P05_MINIO_ACCESS_KEY
  P05_MINIO_SECRET_KEY
  P05_MINIO_CA_CERT_PATH
  P06_DATABASE_URL
  P06_WITNESS_DATABASE_URL
  P06_ALLOW_DATABASE_RESET
  P06_RESET_DATABASE
  P06_WITNESS_RESET_DATABASE
  P07_DATABASE_URL
  P07_WITNESS_DATABASE_URL
  P07_ALLOW_DATABASE_RESET
  P07_RESET_DATABASE
  P07_WITNESS_RESET_DATABASE
  P07_NATS_URL
  P08_DATABASE_URL
  P08_WITNESS_DATABASE_URL
  P08_ALLOW_DATABASE_RESET
  P08_RESET_DATABASE
  P08_WITNESS_RESET_DATABASE
  AR06_ADMIN_DATABASE_URL
  AR06_ADMIN_WITNESS_DATABASE_URL
  AR06_API_DATABASE_URL
  AR06_CANONICAL_DATABASE_URL
  AR06_WITNESS_DATABASE_URL
  AR06_REDIS_URL
  AR06_OPENFGA_ADDRESS
  AR06_OPENFGA_STORE_ID
  AR06_OPENFGA_MODEL_ID
  AR06_OPA_ADDRESS
  AR06_OPA_REVISION
  AR07_DATABASE_URL
  AR07_REALTIME_DATABASE_URL
  AR07_WITNESS_DATABASE_URL
  AR07_NATS_URL
  AR07_DATABASE_NAME
  AR07_WITNESS_DATABASE_NAME
  AR07_ALLOW_DATABASE_RESET
  AR09_AGENT_JOB_FIXTURE_DATABASE_URL
  AR09_AGENT_JOB_API_DATABASE_URL
  AR09_AGENT_JOB_WORKER_DATABASE_URL
  AR09_AGENT_JOB_CANONICAL_DATABASE_URL
  AR09_PUBLIC_FIXTURE_DATABASE_URL
  AR09_PUBLIC_API_DATABASE_URL
  AR09_PUBLIC_WORKER_DATABASE_URL
  AR09_PUBLIC_CANONICAL_DATABASE_URL
  AR09_PUBLIC_WITNESS_DATABASE_URL
  AR09_PUBLIC_REDIS_URL
  TRPG_POSTGRES_CLIENT_IMAGE
  TRPG_POSTGRES_CLIENT_MOUNT_ROOT
  TMPDIR
  SSL_CERT_FILE
  TRPG_REQUIRE_REAL_LOCAL_PROVIDERS
  TRPG_REQUIRE_REAL_CLOUD_PROVIDER
)

export TRPG_REQUIRE_REAL_LOCAL_PROVIDERS="${TRPG_REQUIRE_REAL_LOCAL_PROVIDERS:-0}"
export TRPG_REQUIRE_REAL_CLOUD_PROVIDER="${TRPG_REQUIRE_REAL_CLOUD_PROVIDER:-0}"
case "$TRPG_REQUIRE_REAL_CLOUD_PROVIDER" in
  0) ;;
  1)
    [[ "$TRPG_REQUIRE_REAL_LOCAL_PROVIDERS" == 1 ]] || {
      printf 'real cloud provider verification requires the real provider matrix\n' >&2
      exit 2
    }
    ;;
  *)
    printf 'TRPG_REQUIRE_REAL_CLOUD_PROVIDER must be 0 or 1\n' >&2
    exit 2
    ;;
esac

arguments=()
for key in "${environment_keys[@]}"; do
  arguments+=(--environment-key "$key")
done

report_directory="$(realpath -m -- "$(dirname "$report_path")")"
case "$TRPG_REQUIRE_REAL_LOCAL_PROVIDERS" in
  1)
    provider_environment_keys=(
      TRPG_REAL_PROVIDER_EVIDENCE_DIR
      TRPG_OLLAMA_CHAT_MODEL
      TRPG_OLLAMA_EMBEDDING_MODEL
      OLLAMA_MODELS
      TRPG_LLAMA_SERVER_BIN
      TRPG_LLAMA_CPP_CHAT_MODEL_PATH
      TRPG_LLAMA_CPP_EMBEDDING_MODEL_PATH
    )
    for key in "${provider_environment_keys[@]}"; do
      [[ -n "${!key:-}" ]] || {
        printf 'missing real provider environment variable: %s\n' "$key" >&2
        exit 2
      }
      arguments+=(--environment-key "$key")
    done
    [[ "$(realpath -m -- "$TRPG_REAL_PROVIDER_EVIDENCE_DIR")" == "$report_directory" ]] || {
      printf 'TRPG_REAL_PROVIDER_EVIDENCE_DIR must equal the report directory\n' >&2
      exit 2
    }
    for artifact_name in \
      real-ollama-server.log \
      real-ollama-tls-proxy.log \
      real-llama-cpp-chat.log \
      real-llama-cpp-embedding.log \
      real-llama-cpp-chat-tls-proxy.log \
      real-llama-cpp-embedding-tls-proxy.log; do
      arguments+=(--generated-artifact "$report_directory/$artifact_name")
    done
    case "$TRPG_REQUIRE_REAL_CLOUD_PROVIDER" in
      1)
        cloud_environment_keys=(
          TRPG_REAL_CLOUD_PROVIDER_URL
          TRPG_REAL_CLOUD_CHAT_MODEL
          TRPG_REAL_CLOUD_EMBEDDING_MODEL
          TRPG_REAL_CLOUD_CHAT_MODEL_SHA256
          TRPG_REAL_CLOUD_EMBEDDING_MODEL_SHA256
          TRPG_REAL_CLOUD_CREDENTIAL_PATH
        )
        for key in "${cloud_environment_keys[@]}"; do
          [[ -n "${!key:-}" ]] || {
            printf 'missing real cloud environment variable: %s\n' "$key" >&2
            exit 2
          }
          arguments+=(--environment-key "$key")
        done
        ;;
      0) ;;
      *)
        printf 'TRPG_REQUIRE_REAL_CLOUD_PROVIDER must be 0 or 1\n' >&2
        exit 2
        ;;
    esac
    ;;
  0) ;;
  *)
    printf 'TRPG_REQUIRE_REAL_LOCAL_PROVIDERS must be 0 or 1\n' >&2
    exit 2
    ;;
esac

python3 scripts/ci/generate_evidence.py \
  --report "$report_path" \
  --artifact MANIFEST.md \
  "${arguments[@]}" \
  --service-version-command '["postgres_primary","docker","exec","trpg-primary-postgres","postgres","--version"]' \
  --service-version-command '["postgres_witness","docker","exec","trpg-witness-postgres","postgres","--version"]' \
  --service-version-command '["postgres_tls","docker","exec","trpg-tls-postgres","postgres","--version"]' \
  --service-version-command '["redis","docker","exec","trpg-redis","redis-server","--version"]' \
  --service-version-command '["nats","docker","exec","trpg-nats","nats-server","--version"]' \
  --service-version-command '["openfga","docker","exec","trpg-openfga","/openfga","version"]' \
  --service-version-command '["opa","docker","exec","trpg-opa","/opa","version"]' \
  --service-version-command '["minio","docker","exec","trpg-minio","minio","--version"]' \
  -- \
  bash scripts/ci/test-all.sh
