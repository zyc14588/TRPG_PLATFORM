#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  printf 'usage: %s REPORT_PATH\n' "$0" >&2
  exit 2
fi

report_path="$1"
environment_keys=(
  P02_DATABASE_URL
  P02_REDIS_URL
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
  P02_LIBPQ_SERVICE_FILE
  P02_BACKUP_SOURCE_SERVICE
  P02_BACKUP_TARGET_SERVICE
  P02_BACKUP_SOURCE_URL
  P02_BACKUP_TARGET_URL
  P02_BACKUP_DIR
  P03_DATABASE_URL
  P03_ALLOW_DATABASE_RESET
  P04_DATABASE_URL
  P04_WITNESS_DATABASE_URL
  P04_ALLOW_DATABASE_RESET
  P04_ADMIN_DATABASE_URL
  P04_RECOVERY_DATABASE_URL
  P04_PG_DUMP
  P04_PG_RESTORE
  P05_DATABASE_URL
  P05_WITNESS_DATABASE_URL
  P05_REDIS_URL
  P05_NATS_URL
  P05_MINIO_ENDPOINT
  P05_MINIO_REGION
  P05_MINIO_BUCKET
  P05_MINIO_ACCESS_KEY
  P05_MINIO_SECRET_KEY
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
  TRPG_POSTGRES_CLIENT_IMAGE
  TRPG_POSTGRES_CLIENT_MOUNT_ROOT
  TMPDIR
)

arguments=()
for key in "${environment_keys[@]}"; do
  arguments+=(--environment-key "$key")
done

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
