// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

pub mod schema;

include!("lib_sections/01_module_prelude.rs");
include!("lib_sections/02_persistent_verification_store_new.rs");
include!("lib_sections/03_authentication_context_subject_id.rs");
include!("lib_sections/04_session_record.rs");
include!("lib_sections/05_identity_service_new.rs");
include!("lib_sections/06_identity_service_reload_from_database.rs");
include!("lib_sections/07_identity_service_authenticate_session.rs");
include!("lib_sections/08_identity_service_require_membership.rs");
include!("lib_sections/09_identity_service_verify_signed_credential.rs");
include!("lib_sections/10_service.rs");
