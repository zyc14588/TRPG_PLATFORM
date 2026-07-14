#!/usr/bin/env bash
set -euo pipefail

mode="${1:-all}"
if [[ "$mode" != "all" && "$mode" != "contracts" ]]; then
  echo "usage: $0 [all|contracts]" >&2
  exit 2
fi

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
python3 scripts/ci/validate_workflows.py
python3 scripts/ci/discover_tests.py --check
python3 scripts/ci/verify_test_inventory.py --report "$tool_dir/test-inventory.json"
python3 scripts/ci/manifest.py --check
python3 scripts/ci/verify_evidence_schema.py
python3 scripts/ci/check_dependency_directions.py
python3 scripts/ci/test_dependency_directions.py
python3 scripts/ci/check_product_boundaries.py
python3 scripts/ci/test_product_boundaries.py

bash -n scripts/ci/init-smoke.sh
bash -n scripts/ci/test-all.sh
bash -n scripts/ci/service-process-smoke.sh
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
"$tool_dir/shellcheck-v0.10.0/shellcheck" scripts/ci/*.sh scripts/backup_restore/*.sh scripts/projection_rebuild/*.sh

if [[ "$mode" == "contracts" ]]; then
  python3 scripts/ci/test_repo_truth.py
  exit
fi

cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
npm test
cargo build --workspace --all-targets --release --locked
pnpm --filter ./apps/web... build
pnpm --filter ./apps/web... test
./scripts/ci/service-process-smoke.sh

curl -fsSLo "$tool_dir/opa" https://openpolicyagent.org/downloads/v1.18.2/opa_linux_amd64_static
printf '%s  %s\n' 9903e5125ac281104f2c4b7371d10cc3b74a98933743fcbfc174f9bf0ab20de8 "$tool_dir/opa" | sha256sum -c -
chmod 0755 "$tool_dir/opa"
"$tool_dir/opa" version
"$tool_dir/opa" test policy/opa
