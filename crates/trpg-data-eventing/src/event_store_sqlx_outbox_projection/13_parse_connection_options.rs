
fn parse_connection_options(
    url: &str,
    component: &'static str,
) -> Result<PgConnectOptions, CanonicalStoreError> {
    let options = PgConnectOptions::from_str(url)
        .map_err(|_| CanonicalStoreError::Configuration("invalid_postgresql_url"))?;
    let host = options.get_host();
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with('/');
    if !local && !matches!(options.get_ssl_mode(), PgSslMode::VerifyFull) {
        return Err(CanonicalStoreError::Configuration(match component {
            "primary" => "remote_primary_postgresql_requires_sslmode_verify_full",
            _ => "remote_witness_postgresql_requires_sslmode_verify_full",
        }));
    }
    Ok(options)
}

fn same_endpoint(primary: &PgConnectOptions, witness: &PgConnectOptions) -> bool {
    primary.get_host() == witness.get_host() && primary.get_port() == witness.get_port()
}

fn validate_visibility(label: &str, subject: &str) -> Result<(), CanonicalStoreError> {
    if !matches!(
        label,
        "public"
            | "party_visible"
            | "private_to_player"
            | "private_to_group"
            | "keeper_only"
            | "investigator_private"
            | "ai_internal"
            | "system_only"
            | "spectator_visible"
            | "spectator_hidden"
            | "system_private"
    ) {
        return Err(CanonicalStoreError::Validation("unknown_visibility_label"));
    }
    if matches!(
        label,
        "private_to_player" | "private_to_group" | "investigator_private"
    ) == (subject == "not_applicable")
    {
        return Err(CanonicalStoreError::Validation(
            "visibility_subject_mismatch",
        ));
    }
    Ok(())
}

fn normalize_and_validate(
    draft: &AtomicCommitDraft,
) -> Result<AtomicCommitDraft, CanonicalStoreError> {
    if draft.expected_version < 0 {
        return Err(CanonicalStoreError::Validation(
            "non_negative_expected_version_required",
        ));
    }
    if draft.authority_contract_version <= 0 {
        return Err(CanonicalStoreError::Validation(
            "positive_authority_contract_version_required",
        ));
    }
    if draft.events.is_empty() {
        return Err(CanonicalStoreError::Validation(
            "at_least_one_event_required",
        ));
    }
    if draft.events.len() > 256 {
        return Err(CanonicalStoreError::Validation(
            "event_batch_limit_exceeded",
        ));
    }
    let required = [
        draft.commit_id.as_str(),
        draft.campaign_id.as_str(),
        draft.stream_id.as_str(),
        draft.idempotency_key.as_str(),
        draft.command_id.as_str(),
        draft.authenticated_actor_id.as_str(),
        draft.authenticated_actor_role.as_str(),
        draft.authority_mode.as_str(),
        draft.authority_contract_id.as_str(),
        draft.authority_owner.as_str(),
        draft.visibility_label.as_str(),
        draft.visibility_subject.as_str(),
        draft.data_subject_id.as_str(),
        draft.provenance_kind.as_str(),
        draft.provenance_reference.as_str(),
        draft.provenance_recorded_by.as_str(),
        draft.correlation_id.as_str(),
        draft.causation_id.as_str(),
        draft.trace_id.as_str(),
        draft.audit.actor_id.as_str(),
        draft.audit.actor_origin.as_str(),
        draft.audit.authentication_reference.as_str(),
        draft.audit.resource_type.as_str(),
        draft.audit.resource_id.as_str(),
        draft.audit.action.as_str(),
        draft.audit.requested_role.as_str(),
        draft.audit.openfga_decision_id.as_str(),
        draft.audit.openfga_policy_revision.as_str(),
        draft.audit.opa_decision_id.as_str(),
        draft.audit.opa_policy_revision.as_str(),
    ];
    if required.iter().any(|value| value.trim().is_empty()) {
        return Err(CanonicalStoreError::Validation("required_field_missing"));
    }
    if !matches!(draft.authority_mode.as_str(), "human_kp" | "ai_kp") {
        return Err(CanonicalStoreError::Validation("unknown_authority_mode"));
    }
    if !actor_origin_matches_role_and_campaign(
        &draft.authenticated_actor_role,
        &draft.authenticated_actor_origin,
        &draft.campaign_id,
    ) {
        return Err(CanonicalStoreError::Validation(
            "authenticated_actor_origin_mismatch",
        ));
    }
    if draft.audit.resource_type == "campaign" && draft.audit.resource_id != draft.campaign_id {
        return Err(CanonicalStoreError::Validation(
            "audit_campaign_resource_mismatch",
        ));
    }
    if draft.stream_id != draft.audit.resource_id {
        return Err(CanonicalStoreError::Validation(
            "stream_audit_resource_mismatch",
        ));
    }
    validate_visibility(&draft.visibility_label, &draft.visibility_subject)?;
    if draft.data_subject_id != "not_applicable" && EntityId::new(&draft.data_subject_id).is_err() {
        return Err(CanonicalStoreError::Validation("data_subject_invalid"));
    }
    if !matches!(
        draft.provenance_kind.as_str(),
        "user_statement"
            | "human_keeper_statement"
            | "rules_engine_decision"
            | "tool_result"
            | "agent_proposal"
            | "imported_source"
            | "system_fixture"
    ) {
        return Err(CanonicalStoreError::Validation("unknown_provenance_kind"));
    }

    let mut normalized = draft.clone();
    for event in &mut normalized.events {
        if event.event_type.trim().is_empty() {
            return Err(CanonicalStoreError::Validation("event_type_required"));
        }
        if event.payload_json.len() > 1_048_576 {
            return Err(CanonicalStoreError::Validation(
                "event_payload_limit_exceeded",
            ));
        }
        let value: Value = serde_json::from_str(&event.payload_json)
            .map_err(|_| CanonicalStoreError::Validation("event_payload_must_be_json"))?;
        event.payload_json = serde_json::to_string(&value)
            .map_err(|_| CanonicalStoreError::Validation("event_payload_must_be_json"))?;
        if let Some(visibility) = &event.visibility {
            if draft.audit.resource_type != "campaign_fork"
                || draft.audit.action != "write_official_state"
                || event.event_type != "CampaignForkMaterialized"
            {
                return Err(CanonicalStoreError::Validation(
                    "event_visibility_override_not_allowed",
                ));
            }
            validate_visibility(&visibility.label, &visibility.subject)?;
            if visibility.data_subject_id != "not_applicable"
                && EntityId::new(&visibility.data_subject_id).is_err()
            {
                return Err(CanonicalStoreError::Validation(
                    "event_data_subject_invalid",
                ));
            }
            if matches!(
                visibility.label.as_str(),
                "private_to_player" | "private_to_group" | "investigator_private"
            ) {
                if visibility.data_subject_id != visibility.subject {
                    return Err(CanonicalStoreError::Validation(
                        "private_event_data_subject_mismatch",
                    ));
                }
            } else if visibility.data_subject_id != "not_applicable" {
                return Err(CanonicalStoreError::Validation(
                    "non_private_event_data_subject_mismatch",
                ));
            }
        }
        if event.projection_targets.len() > 32 {
            return Err(CanonicalStoreError::Validation(
                "projection_target_limit_exceeded",
            ));
        }
        let mut unique_targets = BTreeSet::new();
        for target in &event.projection_targets {
            let valid_relation = !target.relation.is_empty()
                && target.relation.len() <= 128
                && target.relation.contains('.')
                && target.relation.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'.')
                });
            if !valid_relation
                || EntityId::new(&target.row_id).is_err()
                || !unique_targets.insert((target.relation.clone(), target.row_id.clone()))
            {
                return Err(CanonicalStoreError::Validation("projection_target_invalid"));
            }
        }
        event.projection_targets.sort_by(|left, right| {
            (&left.relation, &left.row_id).cmp(&(&right.relation, &right.row_id))
        });
    }
    Ok(normalized)
}

fn request_hash_base_fields(draft: &AtomicCommitDraft) -> Vec<String> {
    let mut fields = vec![
        draft.commit_id.clone(),
        draft.campaign_id.clone(),
        draft.stream_id.clone(),
        draft.idempotency_key.clone(),
        draft.expected_version.to_string(),
        draft.command_id.clone(),
        draft.authenticated_actor_id.clone(),
        draft.authenticated_actor_role.clone(),
        draft.authority_mode.clone(),
        draft.authority_contract_version.to_string(),
        draft.authority_contract_id.clone(),
        draft.authority_owner.clone(),
        draft.visibility_label.clone(),
        draft.visibility_subject.clone(),
        draft.data_subject_id.clone(),
        draft.provenance_kind.clone(),
        draft.provenance_reference.clone(),
        draft.provenance_recorded_by.clone(),
        draft.correlation_id.clone(),
        draft.causation_id.clone(),
        draft.trace_id.clone(),
        draft.audit.actor_id.clone(),
        draft.audit.actor_origin.clone(),
        draft.audit.authentication_reference.clone(),
        draft.audit.resource_type.clone(),
        draft.audit.resource_id.clone(),
        draft.audit.action.clone(),
        draft.audit.requested_role.clone(),
        draft.audit.openfga_decision_id.clone(),
        draft.audit.openfga_policy_revision.clone(),
        draft.audit.opa_decision_id.clone(),
        draft.audit.opa_policy_revision.clone(),
        draft.events.len().to_string(),
    ];
    match &draft.authenticated_actor_origin {
        EventActorOriginWire::UserSession { session_id } => {
            fields.push("user_session".to_owned());
            fields.push(session_id.clone());
        }
        EventActorOriginWire::Workload { role } => {
            fields.push("workload".to_owned());
            fields.push(role.clone());
        }
        EventActorOriginWire::AgentRun {
            run_id,
            class,
            campaign_id,
        } => {
            fields.push("agent_run".to_owned());
            fields.push(run_id.clone());
            fields.push(class.clone());
            fields.push(campaign_id.clone());
        }
    }
    fields
}

fn legacy_request_hash_without_projection_targets(draft: &AtomicCommitDraft) -> String {
    let mut fields = request_hash_base_fields(draft);
    for event in &draft.events {
        fields.push(event.event_type.clone());
        fields.push(event.payload_json.clone());
    }
    sha256_fields(&fields)
}

fn request_hash(draft: &AtomicCommitDraft) -> String {
    let mut fields = request_hash_base_fields(draft);
    for event in &draft.events {
        fields.push(event.event_type.clone());
        fields.push(event.payload_json.clone());
        if let Some(visibility) = &event.visibility {
            fields.push("event_visibility_override_v2".to_owned());
            fields.push(visibility.label.clone());
            fields.push(visibility.subject.clone());
            fields.push(visibility.data_subject_id.clone());
        }
        fields.push(event.projection_targets.len().to_string());
        for target in &event.projection_targets {
            fields.push(target.relation.clone());
            fields.push(target.row_id.clone());
        }
    }
    sha256_fields(&fields)
}

fn request_hash_with_event_visibility_v1(draft: &AtomicCommitDraft) -> String {
    let mut fields = request_hash_base_fields(draft);
    for event in &draft.events {
        fields.push(event.event_type.clone());
        fields.push(event.payload_json.clone());
        if let Some(visibility) = &event.visibility {
            fields.push("event_visibility_override_v1".to_owned());
            fields.push(visibility.label.clone());
            fields.push(visibility.subject.clone());
        }
        fields.push(event.projection_targets.len().to_string());
        for target in &event.projection_targets {
            fields.push(target.relation.clone());
            fields.push(target.row_id.clone());
        }
    }
    sha256_fields(&fields)
}

fn stored_request_hash_matches(draft: &AtomicCommitDraft, stored_hash: &str) -> bool {
    stored_hash == request_hash(draft)
        || (draft.events.iter().all(|event| {
            event
                .visibility
                .as_ref()
                .is_none_or(|visibility| visibility.data_subject_id == draft.data_subject_id)
        }) && stored_hash == request_hash_with_event_visibility_v1(draft))
        || (draft
            .events
            .iter()
            .all(|event| event.projection_targets.is_empty() && event.visibility.is_none())
            && stored_hash == legacy_request_hash_without_projection_targets(draft))
}
