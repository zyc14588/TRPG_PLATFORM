
#[test]
fn b024_declares_required_command_event_schema_fields() {
    for required in [
        "command_id",
        "idempotency_key",
        "expected_version",
        "actor",
        "authority_mode",
        "visibility",
        "fact_provenance",
        "correlation_id",
        "causation_id",
        "write_path",
    ] {
        assert!(COMMAND_ENVELOPE_REQUIRED_FIELDS.contains(&required));
    }

    for required in [
        "sequence",
        "event_type",
        "command_id",
        "idempotency_key",
        "visibility",
        "fact_provenance",
        "correlation_id",
        "causation_id",
        "payload",
    ] {
        assert!(EVENT_ENVELOPE_REQUIRED_FIELDS.contains(&required));
    }
}

#[test]
fn b024_declares_current_safe_sqlx_migration_contract() {
    let migrations = persistence_migrations::migrator()
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .collect::<Vec<_>>();
    assert!(migrations.len() >= 7);
    assert_eq!(
        migrations
            .iter()
            .map(|migration| migration.version)
            .collect::<HashSet<_>>()
            .len(),
        migrations.len()
    );

    let event_store = migrations
        .iter()
        .find(|migration| {
            migration.version
                == trpg_data_eventing::sqlx_migrations_contract::FROZEN_EVENT_STORE_MIGRATION_VERSION
        })
        .expect("frozen event-store migration is compiled from migrations/");
    let event_store_sql = event_store.sql.as_ref();
    for required in [
        "event_store",
        "idempotency_key",
        "expected_version",
        "authority_contract_version",
        "visibility_label",
        "fact_provenance_kind",
        "correlation_id",
        "causation_id",
        "UNIQUE",
    ] {
        assert!(event_store_sql.contains(required));
    }
    assert_eq!(
        event_store
            .checksum
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        trpg_data_eventing::sqlx_migrations_contract::FROZEN_EVENT_STORE_MIGRATION_SHA384
    );

    let hardening_sql = migrations
        .iter()
        .find(|migration| migration.version == 20_260_716_000_100)
        .expect("forward-only event persistence hardening migration")
        .sql
        .as_ref();
    for required in [
        "event_outbox",
        "event_sequence",
        "nats_subject",
        "idempotency_key",
        "visibility_label",
        "retry_count",
        "event_outbox_idempotency_scope_uq",
        "event_store_stream_version_uq",
        "event_schema_version",
        "request_hash",
        "request_hash_source",
        "integrity_status",
        "payload_integrity_source",
        "payload_json TYPE JSONB",
        "enforce_event_outbox_binding",
        "nats_subject = 'trpg.events.appended'",
    ] {
        assert!(hardening_sql.contains(required));
    }
    assert!(!hardening_sql.contains("CREATE TABLE IF NOT EXISTS"));
}

fn authority_contract(mode: AuthorityMode) -> AuthorityContract {
    trpg_test_support::authority_contract("campaign_data_eventing_001", mode, 1).unwrap()
}

fn governed_command<T>(
    payload: T,
    expected_version: u64,
    idempotency_key: &str,
    role: ActorRole,
    mode: AuthorityMode,
) -> CommandEnvelope<T> {
    let authority = authority_contract(mode);
    let mut command = trpg_test_support::governed_command_for_contract(&authority, payload, role);
    command.command_id = EntityId::new(format!("command_{idempotency_key}")).unwrap();
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.correlation_id = EntityId::new(format!("corr_{idempotency_key}")).unwrap();
    command.causation_id = EntityId::new(format!("cause_{idempotency_key}")).unwrap();
    command
}
