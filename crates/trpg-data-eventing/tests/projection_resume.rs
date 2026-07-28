// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

mod support;

include!("projection_resume/01_module_prelude.rs");
include!("projection_resume/02_projection_checkpoint_resumes_monotonically_with_stable_protected_hashes.rs");
include!("projection_resume/03_hash_one.rs");
