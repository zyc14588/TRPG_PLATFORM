
impl CoreDomainRepository {
    async fn ensure_user_exists(&self, user_id: &str) -> Result<(), CoreDomainRepositoryError> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.users WHERE user_id = $1)")
                .bind(user_id)
                .fetch_one(&self.primary)
                .await
                .map_err(database_error("load_user"))?;
        if exists {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::NotFound("user"))
        }
    }

    async fn ensure_campaign_member(
        &self,
        campaign_id: &str,
        user_id: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let permitted: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaigns
                 WHERE campaign_id = $1
                   AND owner_user_id = $2
                UNION ALL
                SELECT 1
                  FROM public.campaign_memberships
                 WHERE campaign_id = $1
                   AND user_id = $2
                   AND revoked_at IS NULL
            )
            "#,
        )
        .bind(campaign_id)
        .bind(user_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("authorize_campaign_member"))?;
        if permitted {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::Forbidden)
        }
    }

    async fn ensure_campaign_admin(
        &self,
        campaign_id: &str,
        user_id: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let permitted: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaigns
                 WHERE campaign_id = $1
                   AND owner_user_id = $2
                UNION ALL
                SELECT 1
                  FROM public.campaign_memberships
                 WHERE campaign_id = $1
                   AND user_id = $2
                   AND role IN ('CAMPAIGN_OWNER', 'HUMAN_KEEPER')
                   AND revoked_at IS NULL
            )
            "#,
        )
        .bind(campaign_id)
        .bind(user_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("authorize_campaign_admin"))?;
        if permitted {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::Forbidden)
        }
    }

    async fn ensure_campaign_keeper(
        &self,
        campaign_id: &str,
        user_id: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let permitted: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaign_memberships
                 WHERE campaign_id = $1
                   AND user_id = $2
                   AND role = 'HUMAN_KEEPER'
                   AND revoked_at IS NULL
            )
            "#,
        )
        .bind(campaign_id)
        .bind(user_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("authorize_campaign_keeper"))?;
        if permitted {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::Forbidden)
        }
    }

    async fn projection_matches_command(
        &self,
        event_sequence: i64,
        metadata: &CoreCommandMetadata,
    ) -> Result<bool, CoreDomainRepositoryError> {
        sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.event_store AS event
                  JOIN public.formal_commits AS formal
                    ON event.sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
                 WHERE event.sequence = $1
                   AND event.command_id = $2
                   AND formal.idempotency_key = $3
                   AND formal.commit_id = $4
                   AND formal.campaign_id = event.campaign_id
                   AND formal.stream_id = event.stream_id
                   AND formal.status = 'committed'
                   AND event.integrity_status = 'verified_hmac'
            )
            "#,
        )
        .bind(event_sequence)
        .bind(&metadata.command_id)
        .bind(&metadata.idempotency_key)
        .bind(&metadata.commit_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("verify_projection_command"))
    }

    pub async fn create_campaign(
        &self,
        metadata: &CoreCommandMetadata,
        request: &CreateCampaignRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0 || metadata.requesting_actor_id != request.owner_user_id {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "campaign_create_metadata",
            ));
        }
        let campaign = CampaignAggregate::new(
            &request.campaign_id,
            &request.owner_user_id,
            &request.authority.contract_id,
            &request.title,
            request.created_at_unix_ms,
        )?;
        let room = Room::new(&request.room_id, &request.campaign_id, &request.room_name)?;
        request.authority.validate(&request.campaign_id, metadata)?;
        self.ensure_user_exists(&request.owner_user_id).await?;
        let created_at = timestamp_from_unix_ms(request.created_at_unix_ms, "campaign.created_at")?;
        let event = CoreDomainEvent::CampaignCreated {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            campaign_id: campaign.campaign_id.to_string(),
            owner_user_id: campaign.owner_user_id.to_string(),
            authority_contract_id: request.authority.contract_id.clone(),
            authority_mode: request.authority.authority_mode.clone(),
            authority_owner: request.authority.authority_owner.clone(),
            title: campaign.title.clone(),
            room_id: room.room_id.to_string(),
            room_name: room.name.clone(),
            created_at_unix_ms: request.created_at_unix_ms,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.campaign_id,
                ("campaign", "campaign.create"),
                &event,
                vec![
                    projection_target("public.campaigns", &request.campaign_id),
                    projection_target("public.rooms", &request.room_id),
                ],
            )
            .await?;

        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.campaigns WHERE campaign_id = $1",
        )
        .bind(&request.campaign_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_campaign"))?
        {
            if existing_sequence == persisted.last_event_sequence
                && self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Ok(persisted);
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_identity_conflict",
            ));
        }

        let membership_role = match request.authority.authority_mode.as_str() {
            "HUMAN_KP" => "HUMAN_KEEPER",
            "AI_KP" => "CAMPAIGN_OWNER",
            _ => return Err(CoreDomainRepositoryError::InvalidInput("authority_mode")),
        };
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_campaign_create")
            .await?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_campaign_constraints"))?;
        sqlx::query(
            r#"
            INSERT INTO public.campaigns (
                campaign_id, owner_user_id, authority_contract_id, title,
                state, version, created_at,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, 'DRAFT', 1, $5,
                $6, $7, $8, $9, $10, $11
            )
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(&request.authority.contract_id)
        .bind(&request.title)
        .bind(created_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign"))?;
        sqlx::query(
            r#"
            INSERT INTO public.campaign_memberships (
                campaign_id, user_id, role, granted_by, granted_at
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (campaign_id, user_id) DO NOTHING
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(membership_role)
        .bind(&metadata.requesting_actor_id)
        .bind(created_at)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_owner_membership"))?;
        sqlx::query(
            r#"
            INSERT INTO public.authority_contracts (
                contract_id, campaign_id, authority_mode, authority_owner,
                contract_version, ruleset_version, house_rules_version,
                scenario_version, prompt_version, agent_pack_version,
                tool_schema_version, safety_profile_version,
                ai_provider_snapshot, model_route_snapshot,
                character_sheet_template_version, created_at, locked, change_policy
            ) VALUES (
                $1, $2, $3, $4, 1, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, TRUE, 'FORK_ONLY'
            )
            ON CONFLICT (contract_id) DO NOTHING
            "#,
        )
        .bind(&request.authority.contract_id)
        .bind(&request.campaign_id)
        .bind(&request.authority.authority_mode)
        .bind(&request.authority.authority_owner)
        .bind(&request.authority.ruleset_version)
        .bind(&request.authority.house_rules_version)
        .bind(&request.authority.scenario_version)
        .bind(&request.authority.prompt_version)
        .bind(&request.authority.agent_pack_version)
        .bind(&request.authority.tool_schema_version)
        .bind(&request.authority.safety_profile_version)
        .bind(&request.authority.ai_provider_snapshot)
        .bind(&request.authority.model_route_snapshot)
        .bind(&request.authority.character_sheet_template_version)
        .bind(created_at)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_authority_contract"))?;
        let preprovisioned_identity_matches: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaign_memberships AS membership
                  JOIN public.authority_contracts AS authority
                    ON authority.campaign_id = membership.campaign_id
                 WHERE membership.campaign_id = $1
                   AND membership.user_id = $2
                   AND membership.role = $3
                   AND membership.revoked_at IS NULL
                   AND authority.contract_id = $4
                   AND authority.authority_mode = $5
                   AND authority.authority_owner = $6
                   AND authority.contract_version = 1
                   AND authority.ruleset_version = $7
                   AND authority.house_rules_version = $8
                   AND authority.scenario_version = $9
                   AND authority.prompt_version = $10
                   AND authority.agent_pack_version = $11
                   AND authority.tool_schema_version = $12
                   AND authority.safety_profile_version = $13
                   AND authority.ai_provider_snapshot = $14
                   AND authority.model_route_snapshot = $15
                   AND authority.character_sheet_template_version = $16
                   AND authority.created_at = $17
                   AND authority.locked
                   AND authority.change_policy = 'FORK_ONLY'
            )
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(membership_role)
        .bind(&request.authority.contract_id)
        .bind(&request.authority.authority_mode)
        .bind(&request.authority.authority_owner)
        .bind(&request.authority.ruleset_version)
        .bind(&request.authority.house_rules_version)
        .bind(&request.authority.scenario_version)
        .bind(&request.authority.prompt_version)
        .bind(&request.authority.agent_pack_version)
        .bind(&request.authority.tool_schema_version)
        .bind(&request.authority.safety_profile_version)
        .bind(&request.authority.ai_provider_snapshot)
        .bind(&request.authority.model_route_snapshot)
        .bind(&request.authority.character_sheet_template_version)
        .bind(created_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("verify_campaign_identity_preprovision"))?;
        if !preprovisioned_identity_matches {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_identity_preprovision_mismatch",
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO public.rooms (
                room_id, campaign_id, name, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&request.room_id)
        .bind(&request.campaign_id)
        .bind(&request.room_name)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_room"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_campaign_create"))?;
        Ok(persisted)
    }
}
