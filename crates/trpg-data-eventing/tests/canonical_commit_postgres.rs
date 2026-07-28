// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("canonical_commit_postgres/01_module_prelude.rs");
include!("canonical_commit_postgres/02_canonical_commit_is_atomic_recoverable_and_externally_witnessed.rs");
include!("canonical_commit_postgres/03_canonical_and_witness_endpoints_must_be_distinct.rs");
