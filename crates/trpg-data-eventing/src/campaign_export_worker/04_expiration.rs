impl CampaignExportWorker {
    async fn expire_one(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, CampaignExportWorkerError> {
        let row = sqlx::query(
            r#"
            SELECT export_id, artifact_key
              FROM public.campaign_export_jobs
             WHERE state = 'READY' AND retention_expires_at <= $1
             ORDER BY retention_expires_at, export_id
             LIMIT 1
            "#,
        )
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EXPIRY_LOAD_FAILED"))?;
        let Some(row) = row else { return Ok(None) };
        let export_id: String = row.get("export_id");
        let key: String = row.get("artifact_key");
        remove_artifact(&self.root, &key)?;
        let mut transaction: Transaction<'_, Postgres> =
            self.pool.begin().await.map_err(|_| {
                CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EXPIRY_BEGIN_FAILED")
            })?;
        sqlx::query("DELETE FROM public.campaign_export_download_tickets WHERE export_id = $1")
            .bind(&export_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_TICKET_DELETE_FAILED"))?;
        sqlx::query(
            r#"
            UPDATE public.campaign_export_jobs
               SET state = 'EXPIRED', artifact_key = NULL, deleted_at = $2,
                   lease_owner = NULL, lease_expires_at = NULL, updated_at = $2
             WHERE export_id = $1 AND state = 'READY'
            "#,
        )
        .bind(&export_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EXPIRY_UPDATE_FAILED"))?;
        transaction
            .commit()
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EXPIRY_COMMIT_FAILED"))?;
        Ok(Some(export_id))
    }
}
