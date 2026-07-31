#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'
umask 077

if [[ "$#" -ne 1 ]]; then
  printf 'usage: %s EVIDENCE_DIRECTORY\n' "$0" >&2
  exit 2
fi

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
evidence_directory="$1"
[[ "$evidence_directory" = /* && ! -L "$evidence_directory" ]] || {
  printf 'EVIDENCE_DIRECTORY must be an absolute non-symlink path\n' >&2
  exit 2
}
install -d -m 0700 "$evidence_directory"

export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

AR11_EVIDENCE_DIR="$evidence_directory" \
  bash "$repository_root/apps/web/scripts/live-browser-test.sh"

cargo test -p trpg-testing --test golden_scenarios_ci --all-features --locked
cargo test -p trpg-testing --test model_certification_tests --all-features --locked
cargo test -p trpg-testing --test visibility_leakage --all-features --locked

printf 'authoritative Golden product flow and companion contract tests passed\n'
