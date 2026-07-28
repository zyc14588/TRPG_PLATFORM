// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

pub mod adr_0006_openfga_opa;
pub mod audit_log_contract;
pub mod cloud_egress;
pub mod copyright_boundary;
pub mod data_retention_deletion;
pub mod derived_visibility;
pub mod formal_commit_audit;
pub mod permission_matrix;
pub mod policy_adapter;
pub mod policy_authorization;
pub mod policy_authz;
pub mod policy_openfga_opa;
pub mod privacy_copyright;
pub mod readme;
pub mod secret;
pub mod security_privacy;
pub mod security_privacy_copyright;
pub mod tamper_evident_audit;
pub mod visibility_enforcement_points;

include!("lib_sections/01_module_prelude.rs");
include!("lib_sections/02_audit_sink_authorize_campaign_membership_change.rs");
include!("lib_sections/03_permission_allows.rs");
