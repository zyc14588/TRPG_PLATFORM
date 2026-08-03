impl CoreDomainRepository {
    /// Creates a child campaign and materializes its fork without ever exposing
    /// an empty standalone child projection. Both canonical commits are made
    /// retry-stable first; the campaign, room, lineage, and copied state become
    /// visible together in one projection transaction.
    pub async fn create_forked_campaign(
        &self,
        create_metadata: &CoreCommandMetadata,
        fork_metadata: &CoreCommandMetadata,
        request: &CreateForkedCampaignRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if request.campaign.campaign_id == request.parent_campaign_id
            || create_metadata.requesting_actor_id != fork_metadata.requesting_actor_id
            || create_metadata.authority_contract_id != fork_metadata.authority_contract_id
            || create_metadata.authority_contract_version
                != fork_metadata.authority_contract_version
            || create_metadata.authority_owner != fork_metadata.authority_owner
            || create_metadata.authority_mode != fork_metadata.authority_mode
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "forked_campaign_metadata",
            ));
        }

        // Validate the source and freeze its hash before writing any child
        // event. Later failures leave no child projection; retrying the exact
        // command resumes from the idempotent canonical commits.
        let snapshot = self
            .preview_campaign_fork(
                &request.parent_campaign_id,
                &request.source_session_id,
                &fork_metadata.requesting_actor_id,
            )
            .await?;
        let fork_request = RecordCampaignForkRequest {
            fork_id: request.fork_id.clone(),
            parent_campaign_id: request.parent_campaign_id.clone(),
            child_campaign_id: request.campaign.campaign_id.clone(),
            source_session_id: request.source_session_id.clone(),
            snapshot_hash: snapshot.snapshot_hash,
            reason: request.reason.clone(),
            copy_scopes: snapshot.copy_scopes,
        };

        let existing_child: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM public.campaigns WHERE campaign_id = $1)",
        )
        .bind(&request.campaign.campaign_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("load_existing_forked_campaign"))?;
        if existing_child {
            // Completed retries and legacy create-then-fork callers converge
            // through the existing idempotent projection checks.
            self.create_campaign(create_metadata, &request.campaign)
                .await?;
            return self
                .record_campaign_fork(fork_metadata, &fork_request)
                .await;
        }

        self.validate_pending_campaign_fork_authority(
            &request.parent_campaign_id,
            &request.campaign,
        )
        .await?;
        let created = self
            .commit_campaign_creation_event(create_metadata, &request.campaign)
            .await?;
        let (forked, replay_events) = self
            .commit_campaign_fork_events(
                fork_metadata,
                &fork_request,
                Some(&request.campaign.room_id),
            )
            .await?;

        let mut transaction = self
            .primary
            .begin()
            .await
            .map_err(database_error("begin_forked_campaign_projection"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "p08-campaign-fork-child:{}",
                request.campaign.campaign_id
            ))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_forked_campaign_child"))?;
        self.lock_p08_projection_rebuild_scope(
            &mut transaction,
            &request.campaign.campaign_id,
        )
        .await?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_forked_campaign_constraints"))?;

        self.set_projection_capability(
            &mut transaction,
            &create_metadata.commit_id,
            "set_forked_campaign_create_capability",
        )
        .await?;
        self.project_campaign_creation_in_transaction(
            &mut transaction,
            create_metadata,
            &request.campaign,
            &created,
        )
        .await?;

        self.set_projection_capability(
            &mut transaction,
            &fork_metadata.commit_id,
            "set_forked_campaign_materialization_capability",
        )
        .await?;
        for replay_event in &replay_events {
            apply_campaign_fork_replay_event(&mut transaction, replay_event).await?;
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_forked_campaign_projection"))?;
        Ok(forked)
    }

    async fn validate_pending_campaign_fork_authority(
        &self,
        parent_campaign_id: &str,
        child: &CreateCampaignRequest,
    ) -> Result<(), CoreDomainRepositoryError> {
        let created_at_unix_ms = i64::try_from(child.created_at_unix_ms)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("fork_authority_created_at"))?;
        let valid: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaigns AS parent_campaign
                  JOIN public.authority_contracts AS parent_authority
                    ON parent_authority.contract_id =
                       parent_campaign.authority_contract_id
                   AND parent_authority.campaign_id =
                       parent_campaign.campaign_id
                  JOIN public.authority_contracts AS child_authority
                    ON child_authority.contract_id = $3
                   AND child_authority.campaign_id = $2
                 WHERE parent_campaign.campaign_id = $1
                   AND child_authority.contract_id =
                       'authority_contract_' || $2 || '_1'
                   AND child_authority.contract_id <> parent_authority.contract_id
                   AND child_authority.authority_mode = $4
                   AND child_authority.authority_owner = $5
                   AND child_authority.contract_version = 1
                   AND child_authority.locked
                   AND child_authority.change_policy = 'FORK_ONLY'
                   AND parent_authority.locked
                   AND parent_authority.change_policy = 'FORK_ONLY'
                   AND child_authority.created_at =
                       parent_authority.created_at + INTERVAL '1 millisecond'
                   AND child_authority.created_at =
                       to_timestamp($6::DOUBLE PRECISION / 1000.0)
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
                   ) = ($7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            )
            "#,
        )
        .bind(parent_campaign_id)
        .bind(&child.campaign_id)
        .bind(&child.authority.contract_id)
        .bind(&child.authority.authority_mode)
        .bind(&child.authority.authority_owner)
        .bind(created_at_unix_ms)
        .bind(&child.authority.ruleset_version)
        .bind(&child.authority.house_rules_version)
        .bind(&child.authority.scenario_version)
        .bind(&child.authority.prompt_version)
        .bind(&child.authority.agent_pack_version)
        .bind(&child.authority.tool_schema_version)
        .bind(&child.authority.safety_profile_version)
        .bind(&child.authority.ai_provider_snapshot)
        .bind(&child.authority.model_route_snapshot)
        .bind(&child.authority.character_sheet_template_version)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("validate_pending_campaign_fork_authority"))?;
        if valid {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::InvalidInput(
                "fork_authority_contract",
            ))
        }
    }
}
