#!/usr/bin/env sh
set -eu

cargo test -p trpg-ops --test s10_fixture_acceptance_contract_tests -- backup_restore_smoke_checks_are_executable
