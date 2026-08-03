#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

integration_services_entrypoint_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
integration_services_repository_root="$(cd -- "${integration_services_entrypoint_directory}/../.." && pwd -P)"
# shellcheck source=./scripts/ci/integration-services/01_setup_and_helpers.sh
source "${integration_services_entrypoint_directory}/integration-services/01_setup_and_helpers.sh"
# shellcheck source=./scripts/ci/integration-services/02_execution_and_evidence.sh
source "${integration_services_entrypoint_directory}/integration-services/02_execution_and_evidence.sh"
