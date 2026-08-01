// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("local_model_certification/01_module_prelude.rs");
include!("local_model_certification/02_local_model_certification_authority_new.rs");
include!("local_model_certification/03_registry_ledger_storage.rs");
include!("local_model_certification/04_certification_runner_contracts.rs");
include!("local_model_certification/05_certification_runner_execution.rs");
include!("local_model_certification/06_certification_case_evaluation.rs");
include!("local_model_certification/07_certification_evidence_helpers.rs");
