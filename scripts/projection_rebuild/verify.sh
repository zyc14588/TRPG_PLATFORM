#!/usr/bin/env sh
set -eu

cargo test -p trpg-ops --test s10_fixture_acceptance_contract_tests -- projection_rebuild_verify_checks_are_executable
