// Decomposed to keep each human-maintained source file within the project limit.
mod certification_support;
include!("agent_job_contract_tests/01_provider_and_repository_harness.rs");
include!("agent_job_contract_tests/02_repository_and_decision_ports.rs");
include!("agent_job_contract_tests/03_job_fixtures.rs");
include!("agent_job_contract_tests/04_execution_and_governance_contracts.rs");
include!("agent_job_contract_tests/05_budget_deadline_and_cancellation.rs");
