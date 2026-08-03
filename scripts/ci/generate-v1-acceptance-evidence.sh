#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'
umask 077

if [[ "$#" -ne 4 ]]; then
  printf 'usage: %s EVIDENCE_DIRECTORY RELEASE_EVIDENCE ROW_1_EVIDENCE ROW_14_EVIDENCE\n' "$0" >&2
  exit 2
fi

evidence_directory="$1"
release_evidence="$2"
row_1_evidence="$3"
row_14_evidence="$4"

[[ "$evidence_directory" = /* && ! -L "$evidence_directory" ]] || {
  printf 'EVIDENCE_DIRECTORY must be an absolute non-symlink path\n' >&2
  exit 2
}
install -d -m 0700 "$evidence_directory"

for path in "$release_evidence" "$row_1_evidence" "$row_14_evidence"; do
  [[ "$(dirname "$path")" == "$evidence_directory" && -f "$path" && ! -L "$path" ]] || {
    printf 'input evidence must be a regular sibling file in EVIDENCE_DIRECTORY: %s\n' "$path" >&2
    exit 2
  }
done

while IFS=$'\t' read -r row_id command; do
  if [[ "$row_id" == 1 || "$row_id" == 14 ]]; then
    continue
  fi
  report="$evidence_directory/v1-row-$row_id.json"
  python3 scripts/ci/generate_evidence.py \
    --report "$report" \
    --artifact MANIFEST.md \
    -- bash -lc "$command"
  python3 scripts/ci/verify_evidence_schema.py "$report"
done < <(
  python3 - <<'PY'
import sys

sys.path.insert(0, "scripts/ci")
from acceptance_evidence_matrix_core import acceptance_commands

for row_id, command in acceptance_commands().items():
    print(f"{row_id}\t{command}")
PY
)

python3 scripts/ci/acceptance_evidence_matrix.py derive-batch-closures \
  --evidence "$release_evidence" \
  --output-directory "$evidence_directory"

manifest="$evidence_directory/acceptance-evidence-manifest.json"
matrix="$evidence_directory/V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md"
arguments=(create-final --manifest "$manifest")
for row_id in {1..17}; do
  case "$row_id" in
    1) report="$row_1_evidence" ;;
    14) report="$row_14_evidence" ;;
    *) report="$evidence_directory/v1-row-$row_id.json" ;;
  esac
  arguments+=(--row-evidence "$row_id=$report")
done
for batch_id in RF01 RF02 RF03 RF04; do
  arguments+=(
    --batch-evidence
    "$batch_id=$evidence_directory/${batch_id,,}-closure.json"
  )
done

python3 scripts/ci/acceptance_evidence_matrix.py "${arguments[@]}"
python3 scripts/ci/acceptance_evidence_matrix.py generate \
  --manifest "$manifest" \
  --output "$matrix"
python3 scripts/ci/acceptance_evidence_matrix.py validate \
  --manifest "$manifest" \
  --matrix "$matrix" \
  --require-ready

printf '17-row exact-SHA acceptance evidence generated and verified: %s\n' "$matrix"
