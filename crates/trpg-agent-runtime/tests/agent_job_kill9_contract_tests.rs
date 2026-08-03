#![cfg(unix)]

// Decomposed to keep each human-maintained source file within the project limit.
include!("agent_job_kill9_contract_tests/01_durable_state_harness.rs");
include!("agent_job_kill9_contract_tests/02_file_repository.rs");
include!("agent_job_kill9_contract_tests/03_provider_tool_and_decision_ports.rs");
include!("agent_job_kill9_contract_tests/04_child_process_harness.rs");
include!("agent_job_kill9_contract_tests/05_exactly_once_recovery_contract.rs");
