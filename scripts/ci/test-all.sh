#!/usr/bin/env bash
set -euo pipefail

mode="${1:-all}"
if [[ "$mode" != "all" && "$mode" != "contracts" ]]; then
  echo "usage: $0 [all|contracts]" >&2
  exit 2
fi

# GitHub-hosted runners have materially less scratch space than a full local
# workstation. These settings change only debug/incremental artifacts and
# compilation concurrency; they do not remove packages, targets, features, or
# tests. Keeping them deterministic prevents a disk-exhaustion failure from
# being mistaken for a product-test result.
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

if [[ -n "${RUNNER_TEMP:-}" ]]; then
  tool_dir="$RUNNER_TEMP/p00-tools"
  mkdir -p "$tool_dir"
else
  tool_dir="$(mktemp -d)"
  trap 'rm -rf "$tool_dir"' EXIT
fi

git diff --check
if git rev-parse --verify HEAD^ >/dev/null 2>&1; then
  git diff --check HEAD^ HEAD
else
  git show --check --format= HEAD
fi
test -z "$(git status --porcelain=v1)"

python3 scripts/ci/repo_truth.py --check
python3 scripts/ci/check_source_file_size.py --check
python3 scripts/ci/test_source_file_size.py
python3 scripts/ci/validate_workflows.py
python3 scripts/ci/discover_tests.py --check
python3 scripts/ci/verify_test_inventory.py --report "$tool_dir/test-inventory.json"
python3 scripts/ci/manifest.py --check
python3 scripts/ci/verify_evidence_schema.py
python3 scripts/ci/verify_compose_security.py --check
python3 scripts/ci/check_dependency_directions.py
python3 scripts/ci/test_dependency_directions.py
python3 scripts/ci/check_rustsec_exceptions.py
python3 scripts/ci/check_product_boundaries.py
python3 scripts/ci/test_product_boundaries.py

bash -n scripts/ci/init-smoke.sh
bash -n scripts/ci/test-all.sh
bash -n scripts/ci/service-process-smoke.sh
bash -n scripts/ci/integration-services.sh
bash -n scripts/ci/postgres-container-client.sh
bash -n scripts/ci/p07-integration-services.sh
bash -n scripts/ci/p07-stop-integration-services.sh
bash -n scripts/ci/generate-integration-evidence.sh
bash -n scripts/ci/production-security-smoke.sh
bash -n scripts/ci/production-security-smoke/*.sh
bash -n scripts/backup_restore/smoke.sh
bash -n scripts/projection_rebuild/verify.sh

if command -v pwsh >/dev/null 2>&1; then
  powershell=pwsh
elif command -v powershell.exe >/dev/null 2>&1; then
  powershell=powershell.exe
else
  echo "PowerShell is required" >&2
  exit 1
fi
"$powershell" -NoProfile -File scripts/verify-governance-boundary.ps1
"$powershell" -NoProfile -Command "[scriptblock]::Create((Get-Content -Raw 'scripts/dev/smoke.ps1')) | Out-Null"

curl -fsSLo "$tool_dir/actionlint.tar.gz" https://github.com/rhysd/actionlint/releases/download/v1.7.7/actionlint_1.7.7_linux_amd64.tar.gz
printf '%s  %s\n' 023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757 "$tool_dir/actionlint.tar.gz" | sha256sum -c -
tar -xzf "$tool_dir/actionlint.tar.gz" -C "$tool_dir" actionlint
"$tool_dir/actionlint" .github/workflows/*.yml

curl -fsSLo "$tool_dir/shellcheck.tar.xz" https://github.com/koalaman/shellcheck/releases/download/v0.10.0/shellcheck-v0.10.0.linux.x86_64.tar.xz
printf '%s  %s\n' 6c881ab0698e4e6ea235245f22832860544f17ba386442fe7e9d629f8cbedf87 "$tool_dir/shellcheck.tar.xz" | sha256sum -c -
tar -xJf "$tool_dir/shellcheck.tar.xz" -C "$tool_dir"
ci_shell_scripts=()
for script in scripts/ci/*.sh; do
  if [[ "$script" != "scripts/ci/production-security-smoke.sh" ]]; then
    ci_shell_scripts+=("$script")
  fi
done
"$tool_dir/shellcheck-v0.10.0/shellcheck" \
  "${ci_shell_scripts[@]}" \
  scripts/backup_restore/*.sh \
  scripts/projection_rebuild/*.sh
"$tool_dir/shellcheck-v0.10.0/shellcheck" -x \
  scripts/ci/production-security-smoke.sh

if [[ "$mode" == "contracts" ]]; then
  python3 scripts/ci/test_repo_truth.py
  exit
fi

cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
: "${P02_CANONICAL_DATABASE_URL:?P02_CANONICAL_DATABASE_URL is required for the real PostgreSQL gate}"
: "${P02_CANONICAL_WITNESS_DATABASE_URL:?P02_CANONICAL_WITNESS_DATABASE_URL is required for the real PostgreSQL gate}"
: "${P02_CANONICAL_ALLOW_DATABASE_RESET:?P02_CANONICAL_ALLOW_DATABASE_RESET is required for the destructive canonical gate}"
: "${P02_CANONICAL_RESET_DATABASE:?P02_CANONICAL_RESET_DATABASE is required for the destructive canonical gate}"
: "${P02_CANONICAL_WITNESS_RESET_DATABASE:?P02_CANONICAL_WITNESS_RESET_DATABASE is required for the destructive canonical witness gate}"
: "${P02_EVENTING_DATABASE_URL:?P02_EVENTING_DATABASE_URL is required for the real PostgreSQL/JetStream gate}"
: "${P02_EVENTING_WITNESS_DATABASE_URL:?P02_EVENTING_WITNESS_DATABASE_URL is required for the real PostgreSQL/JetStream gate}"
: "${P02_EVENTING_ALLOW_DATABASE_RESET:?P02_EVENTING_ALLOW_DATABASE_RESET is required for the destructive eventing upgrade gate}"
: "${P02_EVENTING_RESET_DATABASE:?P02_EVENTING_RESET_DATABASE is required for the destructive eventing upgrade gate}"
: "${P02_EVENTING_WITNESS_RESET_DATABASE:?P02_EVENTING_WITNESS_RESET_DATABASE is required for the destructive eventing witness gate}"
: "${P02_NATS_URL:?P02_NATS_URL is required for the real JetStream gate}"
: "${P02_REDIS_URL:?P02_REDIS_URL is required for the real Redis gate}"
: "${P02_DATABASE_URL:?P02_DATABASE_URL is required for the real identity gate}"
: "${P02_TLS_DATABASE_URL:?P02_TLS_DATABASE_URL is required for the real PostgreSQL TLS gate}"
: "${P02_TLS_CA_CERT_PATH:?P02_TLS_CA_CERT_PATH is required for certificate verification}"
: "${P02_PG_DUMP:?P02_PG_DUMP is required for the real backup gate}"
: "${P02_PG_RESTORE:?P02_PG_RESTORE is required for the real restore gate}"
: "${P02_LIBPQ_SERVICE_FILE:?P02_LIBPQ_SERVICE_FILE is required for secret-safe backup connections}"
: "${P02_BACKUP_SOURCE_SERVICE:?P02_BACKUP_SOURCE_SERVICE is required for the real backup gate}"
: "${P02_BACKUP_TARGET_SERVICE:?P02_BACKUP_TARGET_SERVICE is required for the independent restore gate}"
: "${P02_BACKUP_SOURCE_URL:?P02_BACKUP_SOURCE_URL is required for backup verification}"
: "${P02_BACKUP_TARGET_URL:?P02_BACKUP_TARGET_URL is required for restore verification}"
: "${P02_BACKUP_DIR:?P02_BACKUP_DIR is required for the backup artifact}"
: "${P03_DATABASE_URL:?P03_DATABASE_URL is required for the destructive migration gate}"
: "${P03_ALLOW_DATABASE_RESET:?P03_ALLOW_DATABASE_RESET is required for the destructive migration gate}"
: "${P04_DATABASE_URL:?P04_DATABASE_URL is required for the real Event Store gate}"
: "${P04_WITNESS_DATABASE_URL:?P04_WITNESS_DATABASE_URL is required for the independent witness gate}"
: "${P04_ALLOW_DATABASE_RESET:?P04_ALLOW_DATABASE_RESET is required for the destructive Event Store gate}"
: "${P04_ADMIN_DATABASE_URL:?P04_ADMIN_DATABASE_URL is required for the recovery drill}"
: "${P04_RECOVERY_DATABASE_URL:?P04_RECOVERY_DATABASE_URL is required for the independent recovery database}"
: "${P04_PG_DUMP:?P04_PG_DUMP is required for the Event Store recovery drill}"
: "${P04_PG_RESTORE:?P04_PG_RESTORE is required for the Event Store recovery drill}"
: "${P05_DATABASE_URL:?P05_DATABASE_URL is required for the real privacy gate}"
: "${P05_WITNESS_DATABASE_URL:?P05_WITNESS_DATABASE_URL is required for the privacy witness gate}"
: "${P05_REDIS_URL:?P05_REDIS_URL is required for the deletion cache gate}"
: "${P05_NATS_URL:?P05_NATS_URL is required for the deletion queue gate}"
: "${P05_MINIO_ENDPOINT:?P05_MINIO_ENDPOINT is required for the deletion object-store gate}"
: "${P05_MINIO_REGION:?P05_MINIO_REGION is required for the deletion object-store gate}"
: "${P05_MINIO_BUCKET:?P05_MINIO_BUCKET is required for the deletion object-store gate}"
: "${P05_MINIO_ACCESS_KEY:?P05_MINIO_ACCESS_KEY is required for the deletion object-store gate}"
: "${P05_MINIO_SECRET_KEY:?P05_MINIO_SECRET_KEY is required for the deletion object-store gate}"
: "${P06_DATABASE_URL:?P06_DATABASE_URL is required for the real core-domain gate}"
: "${P06_WITNESS_DATABASE_URL:?P06_WITNESS_DATABASE_URL is required for the independent core-domain witness gate}"
: "${P06_ALLOW_DATABASE_RESET:?P06_ALLOW_DATABASE_RESET is required for the destructive core-domain gate}"
: "${P06_RESET_DATABASE:?P06_RESET_DATABASE must name the dedicated core-domain database}"
: "${P06_WITNESS_RESET_DATABASE:?P06_WITNESS_RESET_DATABASE must name the dedicated core-domain witness database}"
: "${P07_DATABASE_URL:?P07_DATABASE_URL is required for the real player-action gate}"
: "${P07_WITNESS_DATABASE_URL:?P07_WITNESS_DATABASE_URL is required for the player-action witness gate}"
: "${P07_ALLOW_DATABASE_RESET:?P07_ALLOW_DATABASE_RESET is required for the destructive player-action gate}"
: "${P07_RESET_DATABASE:?P07_RESET_DATABASE must name the dedicated player-action database}"
: "${P07_WITNESS_RESET_DATABASE:?P07_WITNESS_RESET_DATABASE must name the dedicated player-action witness database}"
: "${P07_NATS_URL:?P07_NATS_URL is required for the real player-action Realtime gate}"
: "${P08_DATABASE_URL:?P08_DATABASE_URL is required for the real Tutorial gate}"
: "${P08_WITNESS_DATABASE_URL:?P08_WITNESS_DATABASE_URL is required for the Tutorial witness gate}"
: "${P08_ALLOW_DATABASE_RESET:?P08_ALLOW_DATABASE_RESET is required for the destructive Tutorial gate}"
: "${P08_RESET_DATABASE:?P08_RESET_DATABASE must name the dedicated Tutorial database}"
: "${P08_WITNESS_RESET_DATABASE:?P08_WITNESS_RESET_DATABASE must name the dedicated Tutorial witness database}"
cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=1
psql "$P03_DATABASE_URL" -X -v ON_ERROR_STOP=1 \
  -f scripts/ci/assert-schema.sql
python3 scripts/ci/p02_boundary_regression.py
npm test
cargo build --workspace --all-targets --release --locked
pnpm --filter ./apps/web... build
pnpm --filter ./apps/web... test
./scripts/ci/service-process-smoke.sh

curl --retry 4 --retry-all-errors --retry-delay 2 -fsSLo "$tool_dir/opa" https://openpolicyagent.org/downloads/v1.18.2/opa_linux_amd64_static
printf '%s  %s\n' 9903e5125ac281104f2c4b7371d10cc3b74a98933743fcbfc174f9bf0ab20de8 "$tool_dir/opa" | sha256sum -c -
chmod 0755 "$tool_dir/opa"
"$tool_dir/opa" version
"$tool_dir/opa" test policy/opa
