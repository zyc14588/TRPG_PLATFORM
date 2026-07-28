// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("jetstream_redis_integration/01_module_prelude.rs");
include!("jetstream_redis_integration/02_outbox_waits_for_jetstream_ack_and_redis_remains_a_versioned_read_model.rs");
