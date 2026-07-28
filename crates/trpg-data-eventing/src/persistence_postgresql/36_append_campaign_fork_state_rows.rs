fn append_campaign_fork_state_rows(
    request: &RecordCampaignForkRequest,
    source: &ForkSnapshotState,
    identity_ids: &BTreeMap<String, String>,
    child_session_id: &str,
    rows: &mut Vec<CampaignForkMaterializedRow>,
) -> Result<(), CoreDomainRepositoryError> {
    for public_event in &source.public_events {
        if public_event.sequence == 0
            || public_event.event_type.trim().is_empty()
            || public_event.resource_type.trim().is_empty()
            || public_event.resource_id.trim().is_empty()
            || !public_event.payload.is_object()
            || !public_event
                .event_integrity_hash
                .starts_with("hmac-sha256:")
            || !matches!(
                public_event.visibility_label.as_str(),
                "public" | "party_visible"
            )
            || public_event.visibility_subject != "not_applicable"
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_public_event_shape",
            ));
        }
        let source_event_sequence = public_event.sequence.to_string();
        rows.push(CampaignForkMaterializedRow::PublicEvent {
            fork_event_id: fork_child_id(
                &request.fork_id,
                "public_event",
                &source_event_sequence,
            )?,
            source_event_sequence: public_event.sequence,
            source_event_type: public_event.event_type.clone(),
            source_resource_type: public_event.resource_type.clone(),
            source_resource_id: public_event.resource_id.clone(),
            source_payload_json: serde_json::to_string(&public_event.payload)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
            source_event_integrity_hash: public_event.event_integrity_hash.clone(),
            visibility_label: public_event.visibility_label.clone(),
            visibility_subject: public_event.visibility_subject.clone(),
        });
    }
    for clue in &source.discovered_clues {
        if clue.clue_id.trim().is_empty()
            || !matches!(clue.importance.as_str(), "CORE" | "OPTIONAL")
            || !matches!(clue.outcome.as_str(), "REVEALED" | "REVEALED_WITH_COST")
            || !matches!(clue.visibility_label.as_str(), "public" | "party_visible")
            || clue.visibility_subject != "not_applicable"
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_clue_snapshot_shape",
            ));
        }
        rows.push(CampaignForkMaterializedRow::DiscoveredClue {
            fork_clue_id: fork_child_id(&request.fork_id, "clue", &clue.clue_id)?,
            source_clue_id: clue.clue_id.clone(),
            importance: clue.importance.clone(),
            outcome: clue.outcome.clone(),
            cost: clue.cost.clone(),
            visibility_label: clue.visibility_label.clone(),
            visibility_subject: clue.visibility_subject.clone(),
        });
    }
    for npc in &source.npc_state {
        if npc.npc_id.trim().is_empty()
            || !npc.state.is_object()
            || !matches!(npc.visibility_label.as_str(), "public" | "party_visible")
            || npc.visibility_subject != "not_applicable"
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_npc_snapshot_shape",
            ));
        }
        rows.push(CampaignForkMaterializedRow::NpcState {
            npc_state_id: identity_ids
                .get(&npc.npc_id)
                .expect("NPC mapping was constructed above")
                .clone(),
            source_npc_id: npc.npc_id.clone(),
            state_json: serde_json::to_string(&npc.state)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
            visibility_label: npc.visibility_label.clone(),
            visibility_subject: npc.visibility_subject.clone(),
        });
    }
    for combat in &source.combat_state {
        if combat.status != "ENDED"
            || combat.round == 0
            || !combat.state.is_object()
            || !matches!(combat.visibility_label.as_str(), "public" | "party_visible")
            || combat.visibility_subject != "not_applicable"
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_combat_snapshot_shape",
            ));
        }
        let child_combat_id = fork_child_id(&request.fork_id, "combat", &combat.combat_id)?;
        let mut state = combat.state.clone();
        rewrite_fork_gameplay_identity_references(&mut state, identity_ids)?;
        state["combat_id"] = Value::String(child_combat_id.clone());
        state["version"] = Value::from(1_u64);
        rows.push(CampaignForkMaterializedRow::Combat {
            combat_id: child_combat_id,
            session_id: child_session_id.to_owned(),
            status: combat.status.clone(),
            round: combat.round,
            current_turn_index: combat.current_turn_index,
            state_json: serde_json::to_string(&state)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
            visibility_label: combat.visibility_label.clone(),
            visibility_subject: combat.visibility_subject.clone(),
        });
    }
    for chase in &source.chase_state {
        if !matches!(chase.status.as_str(), "ESCAPED" | "CAUGHT")
            || chase.range_band > 5
            || chase.segment == 0
            || !chase.state.is_object()
            || !matches!(chase.visibility_label.as_str(), "public" | "party_visible")
            || chase.visibility_subject != "not_applicable"
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_chase_snapshot_shape",
            ));
        }
        let child_chase_id = fork_child_id(&request.fork_id, "chase", &chase.chase_id)?;
        let mut state = chase.state.clone();
        rewrite_fork_gameplay_identity_references(&mut state, identity_ids)?;
        state["chase_id"] = Value::String(child_chase_id.clone());
        state["version"] = Value::from(1_u64);
        rows.push(CampaignForkMaterializedRow::Chase {
            chase_id: child_chase_id,
            session_id: child_session_id.to_owned(),
            status: chase.status.clone(),
            range_band: chase.range_band,
            segment: chase.segment,
            state_json: serde_json::to_string(&state)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
            visibility_label: chase.visibility_label.clone(),
            visibility_subject: chase.visibility_subject.clone(),
        });
    }
    if source.conclusion_state.len() > 1 {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_conclusion_snapshot_shape",
        ));
    }
    for conclusion in &source.conclusion_state {
        if conclusion.ending_id.trim().is_empty()
            || conclusion.summary.trim().is_empty()
            || conclusion.summary.len() > 1_024
            || conclusion.growth_awards.iter().any(|award| {
                award.skill_name.trim().is_empty()
                    || award.skill_name.len() > 256
                    || award.reason.trim().is_empty()
                    || award.reason.len() > 1_024
            })
            || conclusion
                .growth_awards
                .iter()
                .map(|award| award.skill_name.trim())
                .collect::<BTreeSet<_>>()
                .len()
                != conclusion.growth_awards.len()
            || conclusion.ended_at_unix_ms == 0
            || !matches!(
                conclusion.visibility_label.as_str(),
                "public" | "party_visible"
            )
            || conclusion.visibility_subject != "not_applicable"
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_conclusion_snapshot_shape",
            ));
        }
        rows.push(CampaignForkMaterializedRow::Conclusion {
            ending_event_id: fork_child_id(
                &request.fork_id,
                "ending",
                &conclusion.ending_event_id,
            )?,
            session_id: child_session_id.to_owned(),
            ending_id: conclusion.ending_id.clone(),
            summary: conclusion.summary.clone(),
            ended_at_unix_ms: conclusion.ended_at_unix_ms,
            visibility_label: conclusion.visibility_label.clone(),
            visibility_subject: conclusion.visibility_subject.clone(),
        });
    }
    Ok(())
}
