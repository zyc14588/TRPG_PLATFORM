impl CoreDomainRepository {
    pub async fn list_campaigns_for_actor(
        &self,
        actor_id: &str,
        include_all: bool,
    ) -> Result<Vec<CampaignProjection>, CoreDomainRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT campaign.campaign_id, campaign.owner_user_id,
                   campaign.authority_contract_id, campaign.title,
                   campaign.state, campaign.version,
                   campaign.last_event_sequence
              FROM public.campaigns AS campaign
              JOIN public.authority_contracts AS authority
                ON authority.contract_id = campaign.authority_contract_id
               AND authority.campaign_id = campaign.campaign_id
               AND authority.locked
             WHERE $2
                OR EXISTS(
                    SELECT 1
                      FROM public.campaign_memberships AS membership
                     WHERE membership.campaign_id = campaign.campaign_id
                       AND membership.user_id = $1
                       AND membership.revoked_at IS NULL
                )
             ORDER BY campaign.created_at, campaign.campaign_id
            "#,
        )
        .bind(actor_id)
        .bind(include_all)
        .fetch_all(&self.primary)
        .await
        .map_err(database_error("list_campaigns_for_actor"))?;
        Ok(rows.into_iter().map(campaign_projection_from_row).collect())
    }

    pub async fn get_campaign_for_actor(
        &self,
        actor_id: &str,
        include_all: bool,
        campaign_id: &str,
    ) -> Result<CampaignProjection, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT campaign.campaign_id, campaign.owner_user_id,
                   campaign.authority_contract_id, campaign.title,
                   campaign.state, campaign.version,
                   campaign.last_event_sequence
              FROM public.campaigns AS campaign
              JOIN public.authority_contracts AS authority
                ON authority.contract_id = campaign.authority_contract_id
               AND authority.campaign_id = campaign.campaign_id
               AND authority.locked
             WHERE campaign.campaign_id = $3
               AND (
                    $2
                    OR EXISTS(
                        SELECT 1
                          FROM public.campaign_memberships AS membership
                         WHERE membership.campaign_id = campaign.campaign_id
                           AND membership.user_id = $1
                           AND membership.revoked_at IS NULL
                    )
               )
            "#,
        )
        .bind(actor_id)
        .bind(include_all)
        .bind(campaign_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("get_campaign_for_actor"))?
        .ok_or(CoreDomainRepositoryError::NotFound("campaign"))?;
        Ok(campaign_projection_from_row(row))
    }

    pub async fn get_campaign_export_for_actor(
        &self,
        actor_id: &str,
        include_all: bool,
        campaign_id: &str,
        export_id: &str,
    ) -> Result<CampaignExportProjection, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT export.export_id, export.campaign_id, export.requested_by,
                   export.audience, COALESCE(job.state, export.state) AS state,
                   COALESCE(job.attempt_count, 0)::SMALLINT AS attempt_count,
                   5::SMALLINT AS max_attempts, job.failure_code,
                   COALESCE(job.artifact_schema, 'trpg.campaign-export.v1') AS artifact_schema,
                   COALESCE(job.visibility_policy_version, 'visibility-policy-v1')
                       AS visibility_policy_version,
                   job.artifact_hash, job.manifest_hash, job.artifact_size,
                   job.first_event_sequence,
                   job.last_event_sequence AS last_exported_event_sequence,
                   job.event_count,
                   (extract(epoch FROM job.retention_expires_at) * 1000)::BIGINT
                       AS retention_expires_at_unix_ms,
                   fork.fork_id, fork.parent_campaign_id, fork.source_session_id,
                   fork.source_snapshot_hash, fork.child_snapshot_hash,
                   export.version, export.last_event_sequence
              FROM public.campaign_exports AS export
              LEFT JOIN public.campaign_export_jobs AS job
                ON job.export_id = export.export_id
              LEFT JOIN public.campaign_forks AS fork
                ON fork.child_campaign_id = export.campaign_id
             WHERE export.export_id = $4
               AND export.campaign_id = $3
               AND (
                    $2
                    OR (
                        export.audience = 'PLAYER'
                        AND export.requested_by = $1
                    )
                    OR EXISTS(
                        SELECT 1
                          FROM public.campaign_memberships AS membership
                         WHERE membership.campaign_id = export.campaign_id
                           AND membership.user_id = $1
                           AND membership.role IN ('CAMPAIGN_OWNER', 'HUMAN_KEEPER')
                           AND membership.revoked_at IS NULL
                    )
               )
            "#,
        )
        .bind(actor_id)
        .bind(include_all)
        .bind(campaign_id)
        .bind(export_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("get_campaign_export_for_actor"))?
        .ok_or(CoreDomainRepositoryError::NotFound("campaign_export"))?;
        Ok(CampaignExportProjection {
            export_id: row.get("export_id"),
            campaign_id: row.get("campaign_id"),
            requested_by: row.get("requested_by"),
            audience: row.get("audience"),
            state: row.get("state"),
            attempt_count: row.get("attempt_count"),
            max_attempts: row.get("max_attempts"),
            failure_code: row.get("failure_code"),
            artifact_schema: row.get("artifact_schema"),
            visibility_policy_version: row.get("visibility_policy_version"),
            artifact_hash: row.get("artifact_hash"),
            manifest_hash: row.get("manifest_hash"),
            artifact_size: row.get("artifact_size"),
            first_event_sequence: row.get("first_event_sequence"),
            last_exported_event_sequence: row.get("last_exported_event_sequence"),
            event_count: row.get("event_count"),
            retention_expires_at_unix_ms: row.get("retention_expires_at_unix_ms"),
            fork_id: row.get("fork_id"),
            parent_campaign_id: row.get("parent_campaign_id"),
            source_session_id: row.get("source_session_id"),
            source_snapshot_hash: row.get("source_snapshot_hash"),
            child_snapshot_hash: row.get("child_snapshot_hash"),
            aggregate_version: row.get("version"),
            last_event_sequence: row.get("last_event_sequence"),
        })
    }
}
