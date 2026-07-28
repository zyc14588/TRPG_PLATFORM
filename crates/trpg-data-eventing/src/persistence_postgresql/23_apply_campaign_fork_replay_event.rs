async fn apply_campaign_fork_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
        .map_err(|_| CoreDomainRepositoryError::Integrity("campaign_fork_replay_payload"))?;
    event.validate_schema_version()?;
    match event {
        event @ CoreDomainEvent::CampaignForkRecorded { .. } => {
            replay_campaign_fork_recorded(transaction, replay, event).await
        }
        event @ CoreDomainEvent::CampaignForkMaterializationRecorded { .. } => {
            replay_campaign_fork_manifest(transaction, replay, event).await
        }
        event @ CoreDomainEvent::CampaignForkMaterialized { .. } => {
            replay_campaign_fork_batch(transaction, replay, event).await
        }
        _ => Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_replay_event_type",
        )),
    }
}
