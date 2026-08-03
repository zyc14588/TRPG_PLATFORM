#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

service_process_smoke_entrypoint_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
service_process_smoke_repository_root="$(cd -- "${service_process_smoke_entrypoint_directory}/../.." && pwd -P)"
# shellcheck source=./scripts/ci/service-process-smoke/01_setup_and_helpers.sh
source "${service_process_smoke_entrypoint_directory}/service-process-smoke/01_setup_and_helpers.sh"
# shellcheck source=./scripts/ci/service-process-smoke/02_execution_and_evidence.sh
source "${service_process_smoke_entrypoint_directory}/service-process-smoke/02_execution_and_evidence.sh"
