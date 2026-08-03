
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
        let persisted = self
            .commit_campaign_creation_event(metadata, request)
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
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_campaign_create")
            .await?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_campaign_constraints"))?;
        self.project_campaign_creation_in_transaction(
            &mut transaction,
            metadata,
            request,
            &persisted,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_campaign_create"))?;
        Ok(persisted)
    }
}
