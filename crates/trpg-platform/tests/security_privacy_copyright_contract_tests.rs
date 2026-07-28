// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("security_privacy_copyright_contract_tests/01_module_prelude.rs");
include!("security_privacy_copyright_contract_tests/02_trpg_shared_kernel_load_receipt.rs");
include!("security_privacy_copyright_contract_tests/03_canonical_deletion_rejects_an_unwitnessed_hmac_shaped_digest.rs");
