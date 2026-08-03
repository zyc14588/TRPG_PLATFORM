// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("event_store_sqlx_outbox_projection/01_module_prelude.rs");
include!("event_store_sqlx_outbox_projection/02_canonical_event_integrity_record.rs");
include!("event_store_sqlx_outbox_projection/03_fmt_fmt.rs");
include!("event_store_sqlx_outbox_projection/04_witness_phase.rs");
include!("event_store_sqlx_outbox_projection/05_postgres_canonical_store_connect.rs");
include!("event_store_sqlx_outbox_projection/06_postgres_canonical_store_recover.rs");
include!("event_store_sqlx_outbox_projection/07_verify_witness_bindings.rs");
include!("event_store_sqlx_outbox_projection/07_verify_primary_commit.rs");
include!("event_store_sqlx_outbox_projection/07_verify_primary_events.rs");
include!("event_store_sqlx_outbox_projection/07_postgres_canonical_store_verify_integrity.rs");
include!("event_store_sqlx_outbox_projection/08_postgres_canonical_store_load_replay_page.rs");
include!("event_store_sqlx_outbox_projection/08_postgres_canonical_store_replay_bounds.rs");
include!("event_store_sqlx_outbox_projection/09_append_canonical_events.rs");
include!("event_store_sqlx_outbox_projection/09_postgres_canonical_store_commit_primary.rs");
include!("event_store_sqlx_outbox_projection/10_postgres_canonical_store_insert_audit.rs");
include!(
    "event_store_sqlx_outbox_projection/11_postgres_canonical_store_validate_existing_commit.rs"
);
include!("event_store_sqlx_outbox_projection/12_load_committed_events.rs");
include!("event_store_sqlx_outbox_projection/13_parse_connection_options.rs");
include!("event_store_sqlx_outbox_projection/14_actor_origin_matches_role_and_campaign.rs");
include!("event_store_sqlx_outbox_projection/15_field_encoding_prevents_separator_ambiguity.rs");
