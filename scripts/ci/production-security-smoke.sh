#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
runtime_directory="$(mktemp -d)"
secret_directory="$runtime_directory/secrets"
project_name="trpg-security-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}"
swarm_service="${project_name}-secret-canary"
swarm_secret_v1="${project_name}-certificate-v1"
swarm_secret_v2="${project_name}-certificate-v2"
initialized_swarm=false
compose_command=(
  docker compose
  --project-name "$project_name"
  -f "$root/compose.yml"
  -f "$root/docker-compose.ci.yml"
)


# Source ordered phases so credentials and cleanup state stay in one shell.
# shellcheck source=scripts/ci/production-security-smoke/01_preflight_and_helpers.sh
source "$root/scripts/ci/production-security-smoke/01_preflight_and_helpers.sh"
# shellcheck source=scripts/ci/production-security-smoke/02_certificates_and_secrets.sh
source "$root/scripts/ci/production-security-smoke/02_certificates_and_secrets.sh"
# shellcheck source=scripts/ci/production-security-smoke/03_transport_security.sh
source "$root/scripts/ci/production-security-smoke/03_transport_security.sh"
# shellcheck source=scripts/ci/production-security-smoke/04_runtime_and_witness_privileges.sh
source "$root/scripts/ci/production-security-smoke/04_runtime_and_witness_privileges.sh"
# shellcheck source=scripts/ci/production-security-smoke/05_gateway_and_secret_rotation.sh
source "$root/scripts/ci/production-security-smoke/05_gateway_and_secret_rotation.sh"
