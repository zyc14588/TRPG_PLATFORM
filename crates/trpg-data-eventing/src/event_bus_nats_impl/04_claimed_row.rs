
#[cfg(test)]
mod tests {
    use super::*;

    fn claimed_row(integrity_status: &str, request_hash_source: &str) -> OutboxClaim {
        let claimed_at = chrono::Utc::now();
        OutboxClaim {
            outbox_id: 1,
            event_sequence: 1,
            subject: "trpg.events.appended".to_owned(),
            idempotency_key: "claimed_row".to_owned(),
            visibility_label: "party_visible".to_owned(),
            correlation_id: "correlation".to_owned(),
            causation_id: "causation".to_owned(),
            payload_json: serde_json::json!({}),
            commit_id: None,
            event_type: "ClaimedRowProbe".to_owned(),
            event_schema_version: 1,
            campaign_id: "campaign".to_owned(),
            stream_id: "campaign".to_owned(),
            stream_version: 1,
            expected_version: 0,
            event_idempotency_key: "event_claimed_row".to_owned(),
            idempotency_operation: "canonical_commit".to_owned(),
            authenticated_actor_id: "historical_import".to_owned(),
            authenticated_actor_role: "historical_unknown".to_owned(),
            authenticated_actor_origin: sqlx::types::Json(EventActorOriginWire::Workload {
                role: "historical_unknown".to_owned(),
            }),
            resource_type: "campaign".to_owned(),
            resource_id: "campaign".to_owned(),
            authority_contract_id: "historical_authority".to_owned(),
            authority_owner: "historical_owner".to_owned(),
            authority_contract_version: 1,
            command_id: "historical_command".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            data_subject_id: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: "decision".to_owned(),
            provenance_recorded_by: "rules_engine".to_owned(),
            trace_id: "historical_trace".to_owned(),
            recorded_at: claimed_at,
            event_integrity_hash: None,
            request_hash: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            request_hash_source: request_hash_source.to_owned(),
            integrity_status: integrity_status.to_owned(),
            payload_integrity_source: "{}".to_owned(),
            retry_count: 0,
            claimed_at,
            locked_until: claimed_at + chrono::Duration::seconds(60),
            claim_token: "claim-sha256:test:1".to_owned(),
        }
    }

    #[test]
    fn dead_letters_raise_a_stable_operator_alert() {
        let healthy = PublishBatchResult::default();
        assert!(!healthy.requires_operator_attention());
        assert_eq!(healthy.alert_code(), None);
        let failed = PublishBatchResult {
            claimed: 1,
            failed: 1,
            dead_lettered: 1,
            ..PublishBatchResult::default()
        };
        assert!(failed.requires_operator_attention());
        assert_eq!(failed.alert_code(), Some("OUTBOX_DEAD_LETTER_ALERT"));

        let persisted = PublishBatchResult {
            dead_letter_total: 1,
            ..PublishBatchResult::default()
        };
        assert!(persisted.requires_operator_attention());
        assert_eq!(persisted.alert_code(), Some("OUTBOX_DEAD_LETTER_ALERT"));
    }

    #[test]
    fn remote_plaintext_nats_is_rejected() {
        assert_eq!(
            validate_nats_url("nats://nats.example.invalid:4222"),
            Err(JetStreamOutboxError::Configuration(
                "remote_nats_requires_tls"
            ))
        );
        assert_eq!(
            validate_nats_url("tls://nats.example.invalid:4222"),
            Ok((false, true))
        );
        assert_eq!(
            nats_url_credentials("tls://runtime:secret@nats.example.invalid:4222"),
            Ok(Some(("runtime".to_owned(), "secret".to_owned())))
        );
        assert_eq!(
            nats_url_credentials("tls://runtime%20user:secret%40value@nats.example.invalid:4222"),
            Ok(Some(("runtime user".to_owned(), "secret@value".to_owned())))
        );
        assert_eq!(
            nats_endpoint_without_userinfo(
                "tls://runtime%20user:secret%40value@nats.example.invalid:4222"
            ),
            Ok("tls://nats.example.invalid:4222".to_owned())
        );
        assert_eq!(
            nats_url_credentials("tls://runtime@nats.example.invalid:4222"),
            Err(JetStreamOutboxError::Configuration(
                "nats_url_credentials_incomplete"
            ))
        );
    }

    #[test]
    fn outbox_integrity_metadata_rejects_mixed_states() {
        assert!(outbox_integrity_metadata_is_valid(
            "verified_hmac",
            "formal_commit",
            true,
            true,
        ));
        assert!(!outbox_integrity_metadata_is_valid(
            "historical_unsigned",
            "historical_unavailable",
            false,
            false,
        ));
        assert!(!outbox_integrity_metadata_is_valid(
            "historical_unverified_hmac",
            "formal_commit",
            true,
            true,
        ));
        assert!(!outbox_integrity_metadata_is_valid(
            "verified_hmac",
            "historical_unavailable",
            true,
            false,
        ));
        assert!(!outbox_integrity_metadata_is_valid(
            "historical_unsigned",
            "formal_commit",
            false,
            true,
        ));
    }

    #[test]
    fn claimed_row_integrity_failure_is_scoped_to_its_delivery_attempt() {
        let invalid = claimed_row("verified_hmac", "historical_unavailable");
        assert_eq!(
            invalid.validate_for_publish(),
            Err(EventWorkerError::InvalidOutboxPayload)
        );

        let historical = claimed_row("historical_unsigned", "historical_unavailable");
        assert_eq!(
            historical.validate_for_publish(),
            Err(EventWorkerError::InvalidOutboxPayload)
        );
    }

    #[test]
    fn malformed_historical_header_values_return_errors_instead_of_panicking() {
        let valid = claimed_row("historical_unsigned", "historical_unavailable");
        assert!(outbox_headers(&valid).is_ok());

        let mut bad_idempotency = valid.clone();
        bad_idempotency.idempotency_key = "historic\r\nmessage-id".to_owned();
        assert!(matches!(
            outbox_headers(&bad_idempotency),
            Err(JetStreamOutboxError::InvalidOutboxPayload)
        ));

        let mut bad_correlation = valid.clone();
        bad_correlation.correlation_id = "historic\ncorrelation".to_owned();
        assert!(matches!(
            outbox_headers(&bad_correlation),
            Err(JetStreamOutboxError::InvalidOutboxPayload)
        ));

        let mut bad_commit = valid;
        bad_commit.commit_id = Some("historic\rcommit".to_owned());
        assert!(matches!(
            outbox_headers(&bad_commit),
            Err(JetStreamOutboxError::InvalidOutboxPayload)
        ));
    }

    #[test]
    fn jetstream_message_id_binds_the_complete_idempotency_scope() {
        let first = claimed_row("historical_unsigned", "historical_unavailable");
        let mut other_stream = first.clone();
        other_stream.stream_id = "other_stream".to_owned();
        assert_ne!(nats_message_id(&first), nats_message_id(&other_stream));
        assert_eq!(nats_message_id(&first), nats_message_id(&first.clone()));
    }

    #[test]
    fn production_event_envelope_is_versioned_and_never_forges_historical_integrity() {
        let historical = claimed_row("historical_unsigned", "historical_unavailable");
        assert_eq!(
            historical.validate_for_publish(),
            Err(EventWorkerError::InvalidOutboxPayload)
        );

        let mut formal = historical;
        formal.commit_id = Some("formal_commit".to_owned());
        formal.request_hash_source = "formal_commit".to_owned();
        formal.integrity_status = "verified_hmac".to_owned();
        formal.event_integrity_hash = Some(
            "hmac-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        );
        formal.authenticated_actor_id = "keeper".to_owned();
        formal.authenticated_actor_role = "human_keeper".to_owned();
        formal.authenticated_actor_origin = sqlx::types::Json(EventActorOriginWire::UserSession {
            session_id: "session".to_owned(),
        });
        let cipher = PayloadCipher::new("outbox-test-key", &[0x42; 32]).unwrap();
        let encrypted = cipher
            .encrypt_json_field(
                br#"{"clue":"harbor ledger"}"#,
                &[
                    &formal.campaign_id,
                    &formal.stream_id,
                    &formal.command_id,
                    &formal.event_type,
                ],
            )
            .unwrap();
        formal.payload_json = encrypted.envelope().clone();
        formal.payload_integrity_source = serde_json::to_string(encrypted.envelope()).unwrap();
        formal.validate_for_publish().unwrap();
        let envelope = event_envelope(&formal).unwrap();
        assert_eq!(envelope.schema_version, EVENT_ENVELOPE_WIRE_SCHEMA_VERSION);
        assert_eq!(envelope.event_schema_version, 1);
        assert!(matches!(
            envelope.authenticated_actor_origin,
            EventActorOriginWire::UserSession { ref session_id } if session_id == "session"
        ));
        assert_eq!(envelope.request_hash_source, "formal_commit");
        assert_eq!(envelope.integrity_status, "verified_hmac");
        assert_eq!(envelope.integrity_hash, formal.event_integrity_hash);
        assert!(envelope.payload.get("protected_payload").is_some());
    }

    #[test]
    fn publisher_forwards_only_the_protected_payload_envelope() {
        let cipher = PayloadCipher::new("outbox-test-key", &[0x42; 32]).unwrap();
        let mut formal = claimed_row("verified_hmac", "formal_commit");
        formal.commit_id = Some("formal_commit".to_owned());
        formal.event_integrity_hash = Some(
            "hmac-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        );
        let encrypted = cipher
            .encrypt_json_field(
                br#"{"clue":"harbor ledger"}"#,
                &[
                    &formal.campaign_id,
                    &formal.stream_id,
                    &formal.command_id,
                    &formal.event_type,
                ],
            )
            .unwrap();
        formal.payload_json = encrypted.envelope().clone();
        formal.payload_integrity_source = serde_json::to_string(encrypted.envelope()).unwrap();

        formal.validate_for_publish().unwrap();
        let bytes = serde_json::to_vec(&event_envelope(&formal).unwrap()).unwrap();
        let wire: EventEnvelopeWire<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire.payload, formal.payload_json);
        assert!(wire.payload.get("protected_payload").is_some());
        assert!(!String::from_utf8(bytes).unwrap().contains("harbor ledger"));
    }

    #[test]
    fn canonical_delivery_subject_is_data_subject_scoped() {
        let mut claim = claimed_row("verified_hmac", "formal_commit");
        assert_eq!(
            canonical_delivery_subject(&claim),
            "trpg.events.appended.unscoped"
        );
        claim.data_subject_id = "player_subject_123".to_owned();
        assert_eq!(
            canonical_delivery_subject(&claim),
            format!(
                "trpg.events.appended.subject.{:x}",
                Sha256::digest(b"player_subject_123")
            )
        );
    }

    #[test]
    fn every_configured_jetstream_safety_field_is_fail_closed() {
        let desired = canonical_stream_config();
        assert!(stream_config_matches(&desired, &desired));

        let mut server_annotated = desired.clone();
        server_annotated
            .metadata
            .insert("_nats.req.level".to_owned(), "0".to_owned());
        server_annotated
            .metadata
            .insert("_nats.ver".to_owned(), "2.14.3".to_owned());
        server_annotated
            .metadata
            .insert("_nats.level".to_owned(), "4".to_owned());
        assert!(
            stream_config_matches(&server_annotated, &desired),
            "NATS-owned version metadata must not be confused with application policy drift"
        );

        let mut variants = Vec::new();
        let mut changed = desired.clone();
        changed.subjects = vec!["trpg.events.appended".to_owned()];
        variants.push(changed);
        let mut changed = desired.clone();
        changed.max_bytes -= 1;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.max_age -= Duration::from_secs(1);
        variants.push(changed);
        let mut changed = desired.clone();
        changed.duplicate_window -= Duration::from_secs(1);
        variants.push(changed);
        let mut changed = desired.clone();
        changed.storage = StorageType::Memory;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.num_replicas = 2;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.no_ack = true;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.deny_delete = true;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.deny_purge = false;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.retention = async_nats::jetstream::stream::RetentionPolicy::WorkQueue;
        variants.push(changed);
        let mut changed = desired.clone();
        changed
            .metadata
            .insert("owner".to_owned(), "unexpected".to_owned());
        variants.push(changed);
        let mut changed = desired.clone();
        changed
            .metadata
            .insert("_nats.unexpected".to_owned(), "unexpected".to_owned());
        variants.push(changed);
        let mut changed = desired.clone();
        changed.subject_transform = Some(async_nats::jetstream::stream::SubjectTransform {
            source: "trpg.events.>".to_owned(),
            destination: "transformed.>".to_owned(),
        });
        variants.push(changed);
        let mut changed = desired.clone();
        changed.compression = Some(async_nats::jetstream::stream::Compression::S2);
        variants.push(changed);
        let mut changed = desired.clone();
        changed.consumer_limits = Some(async_nats::jetstream::stream::ConsumerLimits {
            inactive_threshold: Duration::from_secs(60),
            max_ack_pending: 32,
        });
        variants.push(changed);
        let mut changed = desired.clone();
        changed.first_sequence = Some(2);
        variants.push(changed);

        assert!(variants
            .iter()
            .all(|actual| !stream_config_matches(actual, &desired)));
    }
}
