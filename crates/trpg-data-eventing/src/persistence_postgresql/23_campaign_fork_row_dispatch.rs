async fn apply_campaign_fork_materialized_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    match row {
        row @ CampaignForkMaterializedRow::Scenario { .. } => {
            apply_fork_scenario_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::Character { .. } => {
            apply_fork_character_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::Session { .. } => {
            apply_fork_session_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::Scene { .. } => {
            apply_fork_scene_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::PublicEvent { .. } => {
            apply_fork_public_event_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::DiscoveredClue { .. } => {
            apply_fork_discovered_clue_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::NpcState { .. } => {
            apply_fork_npc_state_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::Combat { .. } => {
            apply_fork_combat_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::Chase { .. } => {
            apply_fork_chase_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
        row @ CampaignForkMaterializedRow::Conclusion { .. } => {
            apply_fork_conclusion_row(
                transaction,
                replay,
                fork_id,
                child_campaign_id,
                row,
            )
            .await
        }
    }
}
