#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

bootstrap_entrypoint_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
bootstrap_repository_root="$(cd -- "${bootstrap_entrypoint_directory}/../.." && pwd -P)"
# shellcheck source=./scripts/bootstrap/bootstrap/01_setup_and_helpers.sh
source "${bootstrap_entrypoint_directory}/bootstrap/01_setup_and_helpers.sh"
# shellcheck source=./scripts/bootstrap/bootstrap/02_execution_and_evidence.sh
source "${bootstrap_entrypoint_directory}/bootstrap/02_execution_and_evidence.sh"
