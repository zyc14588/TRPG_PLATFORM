
async fn try_insert_historical_probe_event(
    transaction: &mut Transaction<'_, Postgres>,
    idempotency_key: &str,
    stream_version: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, authenticated_actor_role,
            authenticated_actor_origin, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source
        ) VALUES (
            'HistoricalConstraintProbe', 'historical_probe_command', $1, 0,
            'human_kp', 1, 'party_visible', 'imported_source',
            'migration_constraint_probe', 'migration_upgrade',
            'correlation_probe', 'causation_probe', '{}'::jsonb,
            'historical_unscoped', $2, 'historical_unknown', 'historical_unknown',
            '{"kind":"workload","role":"historical_unknown"}'::jsonb,
            'historical_unknown', 'historical_unknown', 'historical_unknown',
            'historical_unknown', 'not_applicable', 'historical_unknown', NULL,
            'historical_unscoped', 1, 'canonical_commit', $3,
            'historical_unavailable', 'historical_unsigned', '{}'
        ) RETURNING sequence
        "#,
    )
    .bind(idempotency_key)
    .bind(stream_version)
    .bind(ZERO_REQUEST_HASH)
    .fetch_one(&mut **transaction)
    .await
}

async fn try_insert_hmac_probe_event(
    transaction: &mut Transaction<'_, Postgres>,
    idempotency_key: &str,
    campaign_id: &str,
    stream_id: &str,
    integrity_status: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, authenticated_actor_role,
            authenticated_actor_origin, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source, payload_ciphertext,
            payload_key_reference, payload_nonce
        ) VALUES (
            'FormalConstraintProbe', 'formal_probe_command', $1, 0,
            'human_kp', 1, 'party_visible', 'rules_engine_decision',
            'migration_constraint_probe', 'migration_upgrade',
            'correlation_probe', 'causation_probe', $6::jsonb, $2, 1,
            'actor_probe', 'workflow',
            '{"kind":"workload","role":"workflow_engine"}'::jsonb,
            'campaign', $2, 'authority_contract_probe',
            'keeper_probe', 'not_applicable', 'trace_probe',
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            $3, 1, 'canonical_commit', $4, 'formal_commit', $5, $6,
            decode(repeat('00', 16), 'hex'), 'migration_fixture_key',
            decode(repeat('00', 12), 'hex')
        ) RETURNING sequence
        "#,
    )
    .bind(idempotency_key)
    .bind(campaign_id)
    .bind(stream_id)
    .bind(REQUEST_HASH_A)
    .bind(integrity_status)
    .bind(PROTECTED_PAYLOAD_FIXTURE)
    .fetch_one(&mut **transaction)
    .await
}

async fn insert_formal_probe_event(
    transaction: &mut Transaction<'_, Postgres>,
    idempotency_key: &str,
    campaign_id: &str,
    stream_id: &str,
) -> i64 {
    try_insert_hmac_probe_event(
        transaction,
        idempotency_key,
        campaign_id,
        stream_id,
        "verified_hmac",
    )
    .await
    .unwrap()
}
