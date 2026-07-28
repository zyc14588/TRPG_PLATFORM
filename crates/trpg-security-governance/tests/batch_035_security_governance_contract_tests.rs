// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

mod common;

const S04_VISIBILITY_ERRORS_FIXTURE: &str =
    include_str!("../../../fixtures/stages/detailed/S04_visibility_policy_errors.current.json.md");
const S04_PERMISSION_MATRIX_FIXTURE: &str =
    include_str!("../../../fixtures/security/permission_matrix.v1.json.md");
const S04_OPENFGA_SECURITY_GOVERNANCE_MODEL: &str =
    include_str!("../../../policy/openfga/security_governance.fga");
const S04_OPENFGA_SECURITY_GOVERNANCE_JSON_MODEL: &str =
    include_str!("../../../policy/openfga/security_governance.model.json");
const S04_VISIBILITY_REDACTION_FIXTURE: &str =
    include_str!("../../../fixtures/visibility/visibility_redaction_matrix.v1.json.md");

include!("batch_035_security_governance_contract_tests/01_module_prelude.rs");
include!(
    "batch_035_security_governance_contract_tests/02_audit_log_contract_persists_audit_metadata.rs"
);
