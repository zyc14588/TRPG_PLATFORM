// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("secret/01_module_prelude.rs");
include!("secret/02_secret_catalog_register.rs");
include!("secret/03_secret_manager_new.rs");
include!("secret/04_secret_catalog_ledger_storage.rs");
include!("secret/05_postgres_ledger_checkpoint.rs");
