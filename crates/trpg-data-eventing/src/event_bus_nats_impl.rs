// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("event_bus_nats_impl/01_outbox_flow_states.rs");
include!("event_bus_nats_impl/02_jet_stream_outbox_publisher_connect.rs");
include!("event_bus_nats_impl/03_stream_config_matches.rs");
include!("event_bus_nats_impl/04_claimed_row.rs");
