pub mod admin_control_plane {
    include!("admin_control_plane/01_types.rs");
    include!("admin_control_plane/02_storage_and_construction.rs");
    include!("admin_control_plane/03_authentication_and_bootstrap.rs");
    include!("admin_control_plane/04_routes_and_audit.rs");
    include!("admin_control_plane/05_operational_mutations.rs");
    include!("admin_control_plane/06_support.rs");

    #[cfg(test)]
    mod tests {
        include!("admin_control_plane/tests.rs");
    }
}
pub mod api_contracts;
pub mod api_contracts_impl;
pub mod background_workers;
pub mod deployment_observability;
pub mod deployment_ops;
pub mod deployment_ops_impl;
pub mod local_dev_environment;
pub mod object_storage;
pub mod observability;
pub mod observability_audit_trace;
pub mod observability_impl;
pub mod performance_budget;
pub mod plugin_sdk;
pub mod plugin_sdk_impl;
pub mod policy_authz;
pub mod policy_authz_impl;
pub mod readme;
pub mod reliability_performance;
pub mod reliability_performance_impl;
pub mod security_privacy_copyright;
pub mod security_privacy_copyrightmpl;

pub use readme::{
    append_platform_event, record_readme_contract, redact_for_observability, restricted_visibility,
    PlatformEvent, PlatformEventEnvelope, PlatformEventStore, RecordReadmeContract,
    PLATFORM_INFRASTRUCTURE_INVARIANTS, PLATFORM_README_RECORDED_EVENT,
};
