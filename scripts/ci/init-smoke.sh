#!/usr/bin/env sh
set -eu

report="${1:-${TMPDIR:-/tmp}/coc-ai-trpg-release-readiness.json}"
python3 scripts/ci/release_readiness.py --report "$report" --require-ready
