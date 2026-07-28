
#[cfg(test)]
mod tests {
    use super::*;
    use trpg_shared_kernel::{CanonicalCommitEvent, CanonicalPolicyAudit};

    #[test]
    fn field_encoding_prevents_separator_ambiguity() {
        assert_ne!(
            sha256_fields(&["a|b".to_owned(), "c".to_owned()]),
            sha256_fields(&["a".to_owned(), "b|c".to_owned()])
        );
    }

    #[test]
    fn canonical_request_maps_the_authorized_resource_to_the_database_stream() {
        let request = CanonicalCommitRequest {
            commit_id: "commit_scene_alpha".to_owned(),
            campaign_id: "campaign_mapping".to_owned(),
            idempotency_key: "mapping_key".to_owned(),
            expected_version: 0,
            command_id: "command_mapping".to_owned(),
            authenticated_actor_id: "workflow_mapping".to_owned(),
            authenticated_actor_role: "workflow".to_owned(),
            authenticated_actor_origin: EventActorOriginWire::Workload {
                role: "workflow_engine".to_owned(),
            },
            authority_mode: "human_kp".to_owned(),
            authority_contract_version: 1,
            authority_contract_id: "authority_mapping".to_owned(),
            authority_owner: "keeper_mapping".to_owned(),
            visibility_label: "party_visible".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            data_subject_id: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: "decision_mapping".to_owned(),
            provenance_recorded_by: "rules_engine_mapping".to_owned(),
            correlation_id: "correlation_mapping".to_owned(),
            causation_id: "causation_mapping".to_owned(),
            trace_id: "trace_mapping".to_owned(),
            events: vec![CanonicalCommitEvent {
                event_type: "SceneAdvanced".to_owned(),
                payload_json: "{}".to_owned(),
            }],
            audit: CanonicalPolicyAudit {
                actor_id: "keeper_mapping".to_owned(),
                actor_origin: "user_session".to_owned(),
                authentication_reference: "session_mapping".to_owned(),
                resource_type: "scene".to_owned(),
                resource_id: "scene_alpha".to_owned(),
                action: "write_official_state".to_owned(),
                requested_role: "human_keeper".to_owned(),
                openfga_decision_id: "fga_mapping".to_owned(),
                openfga_policy_revision: "fga_revision_mapping".to_owned(),
                opa_decision_id: "opa_mapping".to_owned(),
                opa_policy_revision: "opa_revision_mapping".to_owned(),
            },
        };
        let draft = canonical_request_draft(&request).unwrap();
        assert_eq!(draft.campaign_id, "campaign_mapping");
        assert_eq!(draft.stream_id, "scene_alpha");
        assert_eq!(draft.stream_id, draft.audit.resource_id);
        assert!(normalize_and_validate(&draft).is_ok());
    }

    #[test]
    fn zero_target_legacy_request_hash_remains_retry_compatible() {
        let request = CanonicalCommitRequest {
            commit_id: "commit_legacy_retry".to_owned(),
            campaign_id: "campaign_legacy_retry".to_owned(),
            idempotency_key: "legacy_retry_key".to_owned(),
            expected_version: 0,
            command_id: "command_legacy_retry".to_owned(),
            authenticated_actor_id: "workflow_legacy_retry".to_owned(),
            authenticated_actor_role: "workflow".to_owned(),
            authenticated_actor_origin: EventActorOriginWire::Workload {
                role: "workflow_engine".to_owned(),
            },
            authority_mode: "human_kp".to_owned(),
            authority_contract_version: 1,
            authority_contract_id: "authority_legacy_retry".to_owned(),
            authority_owner: "keeper_legacy_retry".to_owned(),
            visibility_label: "party_visible".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            data_subject_id: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: "decision_legacy_retry".to_owned(),
            provenance_recorded_by: "rules_engine_legacy_retry".to_owned(),
            correlation_id: "correlation_legacy_retry".to_owned(),
            causation_id: "causation_legacy_retry".to_owned(),
            trace_id: "trace_legacy_retry".to_owned(),
            events: vec![CanonicalCommitEvent {
                event_type: "SceneAdvanced".to_owned(),
                payload_json: "{}".to_owned(),
            }],
            audit: CanonicalPolicyAudit {
                actor_id: "workflow_legacy_retry".to_owned(),
                actor_origin: "workload".to_owned(),
                authentication_reference: "workflow_legacy_retry".to_owned(),
                resource_type: "scene".to_owned(),
                resource_id: "scene_legacy_retry".to_owned(),
                action: "write_official_state".to_owned(),
                requested_role: "workflow".to_owned(),
                openfga_decision_id: "fga_legacy_retry".to_owned(),
                openfga_policy_revision: "fga_revision_legacy_retry".to_owned(),
                opa_decision_id: "opa_legacy_retry".to_owned(),
                opa_policy_revision: "opa_revision_legacy_retry".to_owned(),
            },
        };
        let mut draft =
            normalize_and_validate(&canonical_request_draft(&request).unwrap()).unwrap();
        let legacy_hash = legacy_request_hash_without_projection_targets(&draft);
        assert_ne!(legacy_hash, request_hash(&draft));
        assert!(stored_request_hash_matches(&draft, &legacy_hash));

        draft.events[0]
            .projection_targets
            .push(CanonicalProjectionTarget {
                relation: "public.scenes".to_owned(),
                row_id: "scene_legacy_retry".to_owned(),
            });
        let draft = normalize_and_validate(&draft).unwrap();
        assert!(!stored_request_hash_matches(&draft, &legacy_hash));

        let mut fork_draft = draft.clone();
        fork_draft.audit.resource_type = "campaign_fork".to_owned();
        fork_draft.events[0].event_type = "CampaignForkMaterialized".to_owned();
        let hash_without_override = request_hash(&normalize_and_validate(&fork_draft).unwrap());
        fork_draft.events[0].visibility = Some(CanonicalEventVisibility {
            label: "private_to_player".to_owned(),
            subject: "player_legacy_retry".to_owned(),
            data_subject_id: "player_legacy_retry".to_owned(),
        });
        let fork_draft = normalize_and_validate(&fork_draft).unwrap();
        assert_ne!(request_hash(&fork_draft), hash_without_override);

        let mut mismatched_subject_draft = fork_draft.clone();
        mismatched_subject_draft.events[0]
            .visibility
            .as_mut()
            .unwrap()
            .data_subject_id = "not_applicable".to_owned();
        assert!(matches!(
            normalize_and_validate(&mismatched_subject_draft),
            Err(CanonicalStoreError::Validation(
                "private_event_data_subject_mismatch"
            ))
        ));

        let mut unrelated_draft = fork_draft;
        unrelated_draft.audit.resource_type = "scene".to_owned();
        assert!(matches!(
            normalize_and_validate(&unrelated_draft),
            Err(CanonicalStoreError::Validation(
                "event_visibility_override_not_allowed"
            ))
        ));
    }

    #[test]
    fn audit_integrity_v1_remains_compatible_with_pre_p03_records() {
        let mut record = AuditRecord {
            sequence: 1,
            commit_id: "success".to_owned(),
            campaign_id: "campaign_atomic_commit".to_owned(),
            actor_id: "keeper_atomic_commit".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_atomic_commit".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: "campaign_atomic_commit".to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            visibility_label: "party_visible".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: "decision_success".to_owned(),
            provenance_recorded_by: "rules_engine_atomic_commit".to_owned(),
            decision: "PERMIT".to_owned(),
            openfga_decision_id: "fga_success".to_owned(),
            openfga_policy_revision: "fga_model_atomic_commit".to_owned(),
            opa_decision_id: "opa_success".to_owned(),
            opa_policy_revision: "opa_bundle_atomic_commit".to_owned(),
            trace_id: "trace_success".to_owned(),
            correlation_id: "correlation_success".to_owned(),
            causation_id: "causation_success".to_owned(),
            event_batch_hash:
                "sha256:f84537114f6cf20ae34cf69c92384ecc45b7247fea73d4aac7deb76eb34d4cc3".to_owned(),
            witness_prepare_sequence: 1,
            witness_prepare_hash:
                "hmac-sha256:426f0375be7bb6ec0632d2cfed79d9c112039bc372f3acf3a7fe9c8b903ad78b"
                    .to_owned(),
            occurred_at: "2026-07-15T16:43:12.930939Z".parse().unwrap(),
            integrity_version: 1,
            key_id: "p02-canonical-test-key".to_owned(),
            previous_hash: GENESIS_HASH.to_owned(),
            record_hash: String::new(),
        };
        let expected =
            "hmac-sha256:c8222f856a745c6527633b334c44b4bd980c95e432d8423656d06f3916bbbbae";
        assert_eq!(audit_record_hash(&[0x9c; 32], &record), expected);

        // v1 intentionally retains its historical input set; timestamp
        // binding begins only at v2 so old signed records do not need re-signing.
        record.occurred_at += chrono::TimeDelta::days(1);
        assert_eq!(audit_record_hash(&[0x9c; 32], &record), expected);
    }

    #[test]
    fn current_audit_hash_binds_correlation_and_causation_without_resigning_history() {
        let mut record = AuditRecord {
            sequence: 1,
            commit_id: "commit_observability".to_owned(),
            campaign_id: "campaign_observability".to_owned(),
            actor_id: "keeper_observability".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_observability".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: "campaign_observability".to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            visibility_label: "keeper_only".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: "decision_observability".to_owned(),
            provenance_recorded_by: "rules_engine_observability".to_owned(),
            decision: "PERMIT".to_owned(),
            openfga_decision_id: "fga_observability".to_owned(),
            openfga_policy_revision: "fga_revision_observability".to_owned(),
            opa_decision_id: "opa_observability".to_owned(),
            opa_policy_revision: "opa_revision_observability".to_owned(),
            trace_id: "trace_observability".to_owned(),
            correlation_id: "correlation_observability".to_owned(),
            causation_id: "causation_observability".to_owned(),
            event_batch_hash:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            witness_prepare_sequence: 1,
            witness_prepare_hash:
                "hmac-sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned(),
            occurred_at: "2026-07-19T00:00:00Z".parse().unwrap(),
            integrity_version: 3,
            key_id: "p04-observability-key".to_owned(),
            previous_hash: GENESIS_HASH.to_owned(),
            record_hash: String::new(),
        };
        let original = audit_record_hash(&[0xd4; 32], &record);
        record.correlation_id = "correlation_tampered".to_owned();
        assert_ne!(audit_record_hash(&[0xd4; 32], &record), original);
        record.correlation_id = "correlation_observability".to_owned();
        record.causation_id = "causation_tampered".to_owned();
        assert_ne!(audit_record_hash(&[0xd4; 32], &record), original);
    }

    #[test]
    fn remote_postgresql_is_fail_closed_without_hostname_verification() {
        let error = parse_connection_options(
            "postgresql://app@example.invalid/trpg?sslmode=require",
            "primary",
        )
        .unwrap_err();
        assert_eq!(
            error,
            CanonicalStoreError::Configuration(
                "remote_primary_postgresql_requires_sslmode_verify_full"
            )
        );
        assert!(parse_connection_options(
            "postgresql://app@example.invalid/trpg?sslmode=verify-full",
            "primary"
        )
        .is_ok());
    }

    #[test]
    fn replay_integrity_metadata_rejects_mixed_states() {
        let verified_hash = format!("hmac-sha256:{}", "a".repeat(64));
        let formal_hash = format!("sha256:{}", "b".repeat(64));
        assert!(replay_integrity_metadata_is_valid(
            "verified_hmac",
            "formal_commit",
            &formal_hash,
            Some(&verified_hash),
            CURRENT_EVENT_INTEGRITY_VERSION,
        ));
        assert!(replay_integrity_metadata_is_valid(
            "historical_unsigned",
            "historical_unavailable",
            ZERO_REQUEST_HASH,
            None,
            0,
        ));
        assert!(replay_integrity_metadata_is_valid(
            "historical_unverified_hmac",
            "formal_commit",
            &formal_hash,
            Some(&verified_hash),
            1,
        ));
        assert!(!replay_integrity_metadata_is_valid(
            "verified_hmac",
            "historical_unavailable",
            ZERO_REQUEST_HASH,
            Some(&verified_hash),
            CURRENT_EVENT_INTEGRITY_VERSION,
        ));
        assert!(!replay_integrity_metadata_is_valid(
            "historical_unsigned",
            "formal_commit",
            &formal_hash,
            None,
            0,
        ));
        assert!(!replay_integrity_metadata_is_valid(
            "unknown",
            "formal_commit",
            &formal_hash,
            Some(&verified_hash),
            CURRENT_EVENT_INTEGRITY_VERSION,
        ));
        assert!(!replay_integrity_metadata_is_valid(
            "verified_hmac",
            "formal_commit",
            &formal_hash,
            Some(&verified_hash),
            1,
        ));
    }
}
