
impl CoreDomainRepository {

    pub async fn preview_campaign_fork(
        &self,
        parent_campaign_id: &str,
        source_session_id: &str,
        requesting_actor_id: &str,
    ) -> Result<CampaignForkSnapshotPreview, CoreDomainRepositoryError> {
        self.ensure_campaign_keeper(parent_campaign_id, requesting_actor_id)
            .await?;
        self.load_public_campaign_fork_snapshot(parent_campaign_id, source_session_id)
            .await
    }
}
