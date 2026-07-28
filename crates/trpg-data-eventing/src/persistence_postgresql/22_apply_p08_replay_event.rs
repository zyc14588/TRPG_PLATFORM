
async fn apply_p08_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
        .map_err(|_| CoreDomainRepositoryError::Integrity("p08_replay_payload"))?;
    event.validate_schema_version()?;
    match &event {
        CoreDomainEvent::CombatStateRecorded { .. } => {
            apply_combat_replay_event(transaction, replay, &event).await
        }
        CoreDomainEvent::ChaseStateRecorded { .. } => {
            apply_chase_replay_event(transaction, replay, &event).await
        }
        CoreDomainEvent::ReconsiderationRequested { .. }
        | CoreDomainEvent::ReconsiderationReviewed { .. }
        | CoreDomainEvent::ReconsiderationUpheld { .. }
        | CoreDomainEvent::ReconsiderationCorrected { .. } => {
            apply_reconsideration_replay_event(transaction, replay, &event).await
        }
        CoreDomainEvent::CampaignForkRecorded { .. }
        | CoreDomainEvent::CampaignForkMaterializationRecorded { .. }
        | CoreDomainEvent::CampaignForkMaterialized { .. } => {
            apply_campaign_fork_replay_event(transaction, replay).await
        }
        CoreDomainEvent::EndingRecorded { .. } => {
            apply_ending_replay_event(transaction, replay, &event).await
        }
        CoreDomainEvent::CharacterGrowthApplied {
            growth_event_id,
            campaign_id,
            session_id,
            ending_event_id,
            character_id,
            source_sheet_version_id,
            new_sheet_version_id,
            skill_name,
            skill_before,
            improvement_check_roll,
            increase_roll,
            skill_after,
            server_roll_id,
            increase_roll_id,
            ..
        } => {
            apply_growth_replay_event(
                transaction,
                replay,
                growth_event_id,
                campaign_id,
                session_id,
                ending_event_id,
                character_id,
                source_sheet_version_id,
                new_sheet_version_id,
                skill_name,
                *skill_before,
                *improvement_check_roll,
                *increase_roll,
                *skill_after,
                server_roll_id,
                increase_roll_id.as_deref(),
            )
            .await
        }
        _ => Err(CoreDomainRepositoryError::Integrity(
            "p08_replay_event_type",
        )),
    }
}
