
impl CoreDomainRepository {

    async fn load_public_campaign_fork_snapshot(
        &self,
        parent_campaign_id: &str,
        source_session_id: &str,
    ) -> Result<CampaignForkSnapshotPreview, CoreDomainRepositoryError> {
        let mut state = load_campaign_fork_snapshot_state(
            &self.primary,
            parent_campaign_id,
            source_session_id,
        )
        .await?;
        let has_nonterminal_combat = state
            .get("combat_state")
            .and_then(Value::as_array)
            .is_some_and(|combats| {
                combats
                    .iter()
                    .any(|combat| combat.get("status").and_then(Value::as_str) != Some("ENDED"))
            });
        let has_nonterminal_chase = state
            .get("chase_state")
            .and_then(Value::as_array)
            .is_some_and(|chases| {
                chases.iter().any(|chase| {
                    !matches!(
                        chase.get("status").and_then(Value::as_str),
                        Some("ESCAPED" | "CAUGHT")
                    )
                })
            });
        if has_nonterminal_combat || has_nonterminal_chase {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "fork_source_gameplay_not_terminal",
            ));
        }
        let cutoff_event_sequence = state
            .get("source_cutoff_event_sequence")
            .and_then(Value::as_i64)
            .filter(|sequence| *sequence > 0)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "fork_snapshot_cutoff_sequence",
            ))?;
        let replay_events = self.load_campaign_events(parent_campaign_id).await?;
        let source_event_sequences = fork_source_session_event_sequences(
            &replay_events,
            parent_campaign_id,
            source_session_id,
            cutoff_event_sequence,
        )?;
        let copyable_base_event_sequences = replay_events
            .iter()
            .filter(|event| {
                source_event_sequences.contains(&event.sequence)
                    && matches!(event.visibility_label.as_str(), "public" | "party_visible")
                    && event.visibility_subject == "not_applicable"
                    && event.integrity_status == "verified_hmac"
                    && event.request_hash_source == "formal_commit"
                    && event.event_integrity_hash.is_some()
            })
            .map(|event| event.sequence)
            .collect::<BTreeSet<_>>();
        let mut relevant_reconsideration_ids = BTreeSet::new();
        for event in replay_events
            .iter()
            .filter(|event| event.event_type == "ReconsiderationRequested")
        {
            let request: CoreDomainEvent =
                serde_json::from_value(event.payload.clone()).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("fork_reconsideration_request_payload")
                })?;
            let CoreDomainEvent::ReconsiderationRequested {
                reconsideration_id,
                original_event_sequence,
                ..
            } = request
            else {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_reconsideration_request_event_type",
                ));
            };
            let original_event_sequence = i64::try_from(original_event_sequence).map_err(|_| {
                CoreDomainRepositoryError::Integrity("fork_reconsideration_original_sequence")
            })?;
            if copyable_base_event_sequences.contains(&original_event_sequence) {
                if event.resource_id != reconsideration_id
                    || !matches!(event.visibility_label.as_str(), "public" | "party_visible")
                    || event.visibility_subject != "not_applicable"
                    || event.integrity_status != "verified_hmac"
                    || event.request_hash_source != "formal_commit"
                    || event.event_integrity_hash.is_none()
                {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_reconsideration_request_integrity",
                    ));
                }
                relevant_reconsideration_ids.insert(reconsideration_id);
            }
        }
        let public_events = replay_events
            .iter()
            .filter(|event| {
                let is_relevant_reconsideration = matches!(
                    event.event_type.as_str(),
                    "ReconsiderationRequested"
                        | "ReconsiderationReviewed"
                        | "ReconsiderationUpheld"
                        | "ReconsiderationCorrected"
                ) && relevant_reconsideration_ids
                    .contains(&event.resource_id);
                (source_event_sequences.contains(&event.sequence) || is_relevant_reconsideration)
                    && matches!(event.visibility_label.as_str(), "public" | "party_visible")
            })
            .map(|event| {
                if event.visibility_subject != "not_applicable"
                    || event.integrity_status != "verified_hmac"
                    || event.request_hash_source != "formal_commit"
                {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_public_event_integrity",
                    ));
                }
                Ok(ForkSnapshotPublicEvent {
                    sequence: u64::try_from(event.sequence).map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_public_event_sequence")
                    })?,
                    event_type: event.event_type.clone(),
                    resource_type: event.resource_type.clone(),
                    resource_id: event.resource_id.clone(),
                    payload: event.payload.clone(),
                    event_integrity_hash: event.event_integrity_hash.clone().ok_or(
                        CoreDomainRepositoryError::Integrity("fork_public_event_integrity_hash"),
                    )?,
                    visibility_label: event.visibility_label.clone(),
                    visibility_subject: event.visibility_subject.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        state["public_events"] = serde_json::to_value(public_events)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        state["character_state"] = serde_json::to_value(reconstruct_fork_characters(
            &replay_events,
            parent_campaign_id,
            &source_event_sequences,
        )?)
        .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        state["npc_state"] = Value::Array(derive_fork_gameplay_npc_state(&state)?);
        let snapshot = serde_json::json!({
            "schema_version": 1,
            "copy_scopes": DEFAULT_PUBLIC_COPY_SCOPES,
            "excluded_private_scopes": [
                CopyScope::KeeperNotes,
                CopyScope::HiddenClues,
                CopyScope::PrivateMessages,
                CopyScope::AiInternalMemory
            ],
            "state": state
        });
        let canonical_snapshot_json = serde_json::to_string(&snapshot)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let snapshot_hash = format!(
            "sha256:{:x}",
            Sha256::digest(canonical_snapshot_json.as_bytes())
        );
        Ok(CampaignForkSnapshotPreview {
            canonical_snapshot_json,
            snapshot_hash,
            copy_scopes: DEFAULT_PUBLIC_COPY_SCOPES.to_vec(),
        })
    }
}
