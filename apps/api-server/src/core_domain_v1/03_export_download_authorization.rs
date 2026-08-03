macro_rules! lifecycle_export_download_methods {
    () => {
    fn issue_campaign_export_download<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
        campaign_id: &'a str,
        export_id: &'a str,
        now_unix_ms: u64,
    ) -> CoreApiFuture<'a, CampaignExportDownloadAuthorizationApiResponse> {
        Box::pin(async move {
            self.repository
                .issue_campaign_export_download_for_actor(
                    actor_id,
                    include_all,
                    campaign_id,
                    export_id,
                    now_unix_ms,
                    60_000,
                )
                .await
                .map(|authorization| CampaignExportDownloadAuthorizationApiResponse {
                    token: authorization.token,
                    expires_at_unix_ms: authorization.expires_at_unix_ms,
                    download_path: format!(
                        "/api/v1/campaigns/{campaign_id}/exports/{export_id}/download"
                    ),
                })
                .map_err(Self::map_error)
        })
    }

    fn consume_campaign_export_download<'a>(
        &'a self,
        actor_id: &'a str,
        campaign_id: &'a str,
        export_id: &'a str,
        token: &'a str,
        now_unix_ms: u64,
    ) -> CoreApiFuture<'a, CampaignExportDownloadDescriptor> {
        Box::pin(async move {
            self.repository
                .consume_campaign_export_download_for_actor(
                    actor_id,
                    campaign_id,
                    export_id,
                    token,
                    now_unix_ms,
                )
                .await
                .map(|CampaignExportDownloadArtifact {
                    artifact_key,
                    artifact_hash,
                }| CampaignExportDownloadDescriptor {
                    artifact_key,
                    artifact_hash,
                })
                .map_err(Self::map_error)
        })
    }
    };
}
