impl CoreDomainRepository {
    async fn validate_campaign_fork_authority(
        &self,
        request: &RecordCampaignForkRequest,
    ) -> Result<(), CoreDomainRepositoryError> {
        let child_authority_is_derived: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaigns AS parent_campaign
                  JOIN public.authority_contracts AS parent_authority
                    ON parent_authority.contract_id =
                       parent_campaign.authority_contract_id
                   AND parent_authority.campaign_id =
                       parent_campaign.campaign_id
                  JOIN public.campaigns AS child_campaign
                    ON child_campaign.campaign_id = $2
                  JOIN public.authority_contracts AS child_authority
                    ON child_authority.contract_id =
                       child_campaign.authority_contract_id
                   AND child_authority.campaign_id =
                       child_campaign.campaign_id
                 WHERE parent_campaign.campaign_id = $1
                   AND child_authority.contract_id =
                       'authority_contract_' || child_campaign.campaign_id || '_1'
                   AND child_authority.contract_id <>
                       parent_authority.contract_id
                   AND child_authority.contract_version = 1
                   AND child_authority.locked
                   AND child_authority.change_policy = 'FORK_ONLY'
                   AND parent_authority.locked
                   AND parent_authority.change_policy = 'FORK_ONLY'
                   AND child_authority.created_at =
                       parent_authority.created_at + INTERVAL '1 millisecond'
                   AND (
                       child_authority.ruleset_version,
                       child_authority.house_rules_version,
                       child_authority.scenario_version,
                       child_authority.prompt_version,
                       child_authority.agent_pack_version,
                       child_authority.tool_schema_version,
                       child_authority.safety_profile_version,
                       child_authority.ai_provider_snapshot,
                       child_authority.model_route_snapshot,
                       child_authority.character_sheet_template_version
                   ) = (
                       parent_authority.ruleset_version,
                       parent_authority.house_rules_version,
                       parent_authority.scenario_version,
                       parent_authority.prompt_version,
                       parent_authority.agent_pack_version,
                       parent_authority.tool_schema_version,
                       parent_authority.safety_profile_version,
                       parent_authority.ai_provider_snapshot,
                       parent_authority.model_route_snapshot,
                       parent_authority.character_sheet_template_version
                   )
            )
            "#,
        )
        .bind(&request.parent_campaign_id)
        .bind(&request.child_campaign_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("validate_campaign_fork_authority"))?;
        if !child_authority_is_derived {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "fork_authority_contract",
            ));
        }
        Ok(())
    }

    async fn project_campaign_fork_replay(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordCampaignForkRequest,
        replay_events: &[CanonicalReplayEvent],
    ) -> Result<(), CoreDomainRepositoryError> {
        // Do not reserve a pool connection while snapshot construction,
        // canonical commit, or replay-page decryption may themselves need a
        // connection. The canonical event-store uniqueness invariant prevents
        // two lineages for one child campaign; this short transaction only
        // serializes and atomically applies the child projection.
        let mut child_lineage_guard = self
            .primary
            .begin()
            .await
            .map_err(database_error("begin_campaign_fork_child_lock"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "p08-campaign-fork-child:{}",
                request.child_campaign_id
            ))
            .execute(&mut *child_lineage_guard)
            .await
            .map_err(database_error("lock_campaign_fork_child"))?;
        self.lock_p08_projection_rebuild_scope(
            &mut child_lineage_guard,
            &request.child_campaign_id,
        )
        .await?;
        self.set_projection_capability(
            &mut child_lineage_guard,
            &metadata.commit_id,
            "set_campaign_fork_projection_capability",
        )
        .await?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *child_lineage_guard)
            .await
            .map_err(database_error("defer_campaign_fork_constraints"))?;
        for replay_event in replay_events {
            apply_campaign_fork_replay_event(&mut child_lineage_guard, replay_event).await?;
        }
        child_lineage_guard
            .commit()
            .await
            .map_err(database_error("commit_campaign_fork_child_lock"))?;
        Ok(())
    }
}
