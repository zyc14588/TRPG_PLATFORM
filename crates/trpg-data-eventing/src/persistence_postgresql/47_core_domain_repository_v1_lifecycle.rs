#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateCharacterRequest {
    pub character_id: String,
    pub campaign_id: String,
    pub owner_user_id: String,
    pub display_name: String,
    pub sheet_version_id: String,
    pub sheet_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinCharacterSessionRequest {
    pub join_id: String,
    pub campaign_id: String,
    pub session_id: String,
    pub character_id: String,
    pub owner_user_id: String,
    pub joined_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestCampaignExportRequest {
    pub export_id: String,
    pub campaign_id: String,
    pub requested_by: String,
    pub audience: String,
    pub requested_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignProjection {
    pub campaign_id: String,
    pub owner_user_id: String,
    pub authority_contract_id: String,
    pub title: String,
    pub state: String,
    pub aggregate_version: i64,
    pub last_event_sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignExportProjection {
    pub export_id: String,
    pub campaign_id: String,
    pub requested_by: String,
    pub audience: String,
    pub state: String,
    pub attempt_count: i16,
    pub max_attempts: i16,
    pub failure_code: Option<String>,
    pub artifact_schema: String,
    pub visibility_policy_version: String,
    pub artifact_hash: Option<String>,
    pub manifest_hash: Option<String>,
    pub artifact_size: Option<i64>,
    pub first_event_sequence: Option<i64>,
    pub last_exported_event_sequence: Option<i64>,
    pub event_count: Option<i64>,
    pub retention_expires_at_unix_ms: Option<i64>,
    pub fork_id: Option<String>,
    pub parent_campaign_id: Option<String>,
    pub source_session_id: Option<String>,
    pub source_snapshot_hash: Option<String>,
    pub child_snapshot_hash: Option<String>,
    pub aggregate_version: i64,
    pub last_event_sequence: i64,
}

impl CoreDomainRepository {
    pub async fn update_character(
        &self,
        metadata: &CoreCommandMetadata,
        request: &UpdateCharacterRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.requesting_actor_id != request.owner_user_id
            || request.display_name.trim().is_empty()
            || request.display_name.len() > 512
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_update",
            ));
        }
        let sheet_json = validated_object(&request.sheet_json, "character.sheet_json")?;
        self.ensure_campaign_member(&request.campaign_id, &request.owner_user_id)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT campaign_id, owner_user_id, state, current_sheet_version,
                   initial_version_locked, version, last_event_sequence
              FROM public.characters
             WHERE character_id = $1
            "#,
        )
        .bind(&request.character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_for_update"))?
        .ok_or(CoreDomainRepositoryError::NotFound("character"))?;
        if row.get::<String, _>("campaign_id") != request.campaign_id
            || row.get::<String, _>("owner_user_id") != request.owner_user_id
        {
            return Err(CoreDomainRepositoryError::Forbidden);
        }

        let event = CoreDomainEvent::CharacterUpdated {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            character_id: request.character_id.clone(),
            campaign_id: request.campaign_id.clone(),
            display_name: request.display_name.trim().to_owned(),
            sheet_version_id: request.sheet_version_id.clone(),
            sheet_json: serde_json::to_string(&sheet_json)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
        };
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if self
            .projection_matches_command(current_event_sequence, metadata)
            .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.character_id,
                    metadata,
                    "CharacterUpdated",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_character_update_missing",
                ))?;
            if existing_event != event {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_character_update_conflict",
                ));
            }
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.character_id,
                    ("character", "character.update"),
                    &event,
                    vec![
                        projection_target("public.characters", &request.character_id),
                        projection_target(
                            "public.character_sheet_versions",
                            &request.sheet_version_id,
                        ),
                    ],
                )
                .await;
        }

        let current_version: i64 = row.get("version");
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_expected_version",
            ));
        }
        if row.get::<String, _>("state") != "DRAFT"
            || row.get::<bool, _>("initial_version_locked")
        {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::CharacterSheetAlreadyLocked,
            ));
        }
        let next_sheet_version = row
            .get::<i64, _>("current_sheet_version")
            .checked_add(1)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "character_sheet_version_overflow",
            ))?;
        let sheet_identity_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM public.character_sheet_versions \
             WHERE sheet_version_id = $1)",
        )
        .bind(&request.sheet_version_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("check_character_sheet_identity"))?;
        if sheet_identity_exists {
            return Err(CoreDomainRepositoryError::Integrity(
                "character_sheet_identity_conflict",
            ));
        }

        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.character_id,
                ("character", "character.update"),
                &event,
                vec![
                    projection_target("public.characters", &request.character_id),
                    projection_target(
                        "public.character_sheet_versions",
                        &request.sheet_version_id,
                    ),
                ],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_update")
            .await?;
        sqlx::query(
            r#"
            INSERT INTO public.character_sheet_versions (
                sheet_version_id, character_id, version, sheet_json, locked,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                campaign_id, last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, FALSE, $5, $6, $7, $8, $9, $10, $11
            )
            "#,
        )
        .bind(&request.sheet_version_id)
        .bind(&request.character_id)
        .bind(next_sheet_version)
        .bind(sqlx::types::Json(sheet_json))
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(&request.campaign_id)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_updated_character_sheet"))?;
        let result = sqlx::query(
            r#"
            UPDATE public.characters
               SET display_name = $1,
                   current_sheet_version = $2,
                   version = version + 1,
                   visibility_label = $3,
                   visibility_subject = $4,
                   provenance_kind = $5,
                   provenance_reference = $6,
                   provenance_recorded_by = $7,
                   last_event_sequence = $8
             WHERE character_id = $9
               AND campaign_id = $10
               AND owner_user_id = $11
               AND state = 'DRAFT'
               AND initial_version_locked = FALSE
               AND version = $12
            "#,
        )
        .bind(request.display_name.trim())
        .bind(next_sheet_version)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_character_update"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "character_update_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_update"))?;
        Ok(persisted)
    }

    pub async fn join_character_session(
        &self,
        metadata: &CoreCommandMetadata,
        request: &JoinCharacterSessionRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || metadata.requesting_actor_id != request.owner_user_id
            || request.joined_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_session_join",
            ));
        }
        self.ensure_campaign_member(&request.campaign_id, &request.owner_user_id)
            .await?;
        let joined_at =
            timestamp_from_unix_ms(request.joined_at_unix_ms, "character_session.joined_at")?;
        let state = sqlx::query(
            r#"
            SELECT character.campaign_id AS character_campaign_id,
                   character.owner_user_id, character.state AS character_state,
                   session.campaign_id AS session_campaign_id,
                   session.state AS session_state
              FROM public.characters AS character
              JOIN core_domain.sessions AS session
                ON session.session_id = $2
             WHERE character.character_id = $1
            "#,
        )
        .bind(&request.character_id)
        .bind(&request.session_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_session_join_state"))?
        .ok_or(CoreDomainRepositoryError::NotFound(
            "character_or_session",
        ))?;
        if state.get::<String, _>("character_campaign_id") != request.campaign_id
            || state.get::<String, _>("session_campaign_id") != request.campaign_id
            || state.get::<String, _>("owner_user_id") != request.owner_user_id
        {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        if state.get::<String, _>("character_state") != "APPROVED"
            || !matches!(
                state.get::<String, _>("session_state").as_str(),
                "ACTIVE" | "PAUSED"
            )
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_session_join_state",
            ));
        }

        let event = CoreDomainEvent::CharacterJoinedSession {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            join_id: request.join_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            character_id: request.character_id.clone(),
            owner_user_id: request.owner_user_id.clone(),
            joined_at_unix_ms: request.joined_at_unix_ms,
        };
        let existing = sqlx::query(
            r#"
            SELECT join_id, session_id, character_id, owner_user_id,
                   last_event_sequence
              FROM core_domain.session_characters
             WHERE join_id = $1
                OR (session_id = $2 AND character_id = $3)
                OR (session_id = $2 AND owner_user_id = $4)
            "#,
        )
        .bind(&request.join_id)
        .bind(&request.session_id)
        .bind(&request.character_id)
        .bind(&request.owner_user_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_character_session_join"))?;
        if let Some(existing) = existing {
            let sequence: i64 = existing.get("last_event_sequence");
            if existing.get::<String, _>("join_id") == request.join_id
                && existing.get::<String, _>("session_id") == request.session_id
                && existing.get::<String, _>("character_id") == request.character_id
                && existing.get::<String, _>("owner_user_id") == request.owner_user_id
                && self.projection_matches_command(sequence, metadata).await?
            {
                let existing_event = self
                    .load_idempotent_core_event(
                        &request.campaign_id,
                        &request.join_id,
                        metadata,
                        "CharacterJoinedSession",
                    )
                    .await?
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "idempotent_character_session_join_missing",
                    ))?;
                if existing_event != event {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "idempotent_character_session_join_conflict",
                    ));
                }
                return self
                    .commit_event(
                        metadata,
                        &request.campaign_id,
                        &request.join_id,
                        ("session_character", "session_character.join"),
                        &event,
                        vec![projection_target(
                            "core_domain.session_characters",
                            &request.join_id,
                        )],
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "character_session_join_conflict",
            ));
        }

        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.join_id,
                ("session_character", "session_character.join"),
                &event,
                vec![projection_target(
                    "core_domain.session_characters",
                    &request.join_id,
                )],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_session_join")
            .await?;
        sqlx::query(
            r#"
            INSERT INTO core_domain.session_characters (
                join_id, campaign_id, session_id, character_id,
                owner_user_id, joined_by, joined_at, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $5, $6, 1,
                $7, $8, $9, $10, $11, $12
            )
            "#,
        )
        .bind(&request.join_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(&request.character_id)
        .bind(&request.owner_user_id)
        .bind(joined_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_character_session_join"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_session_join"))?;
        Ok(persisted)
    }

    pub async fn request_campaign_export(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RequestCampaignExportRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || metadata.requesting_actor_id != request.requested_by
            || !matches!(
                request.audience.as_str(),
                "PLAYER" | "KEEPER_PRIVATE" | "AUDIT" | "CAMPAIGN_ARCHIVE"
            )
            || request.requested_at_unix_ms == 0
            || (request.audience == "PLAYER"
                && (metadata.visibility_label != "private_to_player"
                    || metadata.visibility_subject != request.requested_by))
            || (request.audience != "PLAYER" && metadata.visibility_label != "keeper_only")
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "campaign_export_request",
            ));
        }
        if request.audience == "PLAYER" {
            self.ensure_campaign_member(&request.campaign_id, &request.requested_by)
                .await?;
        } else {
            self.ensure_campaign_admin(&request.campaign_id, &request.requested_by)
                .await?;
        }
        let requested_at =
            timestamp_from_unix_ms(request.requested_at_unix_ms, "campaign_export.requested_at")?;
        let event = CoreDomainEvent::CampaignExportRequested {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            export_id: request.export_id.clone(),
            campaign_id: request.campaign_id.clone(),
            requested_by: request.requested_by.clone(),
            audience: request.audience.clone(),
            requested_at_unix_ms: request.requested_at_unix_ms,
        };
        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.campaign_exports WHERE export_id = $1",
        )
        .bind(&request.export_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_campaign_export"))?
        {
            if self
                .projection_matches_command(existing_sequence, metadata)
                .await?
            {
                let existing_event = self
                    .load_idempotent_core_event(
                        &request.campaign_id,
                        &request.export_id,
                        metadata,
                        "CampaignExportRequested",
                    )
                    .await?
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "idempotent_campaign_export_missing",
                    ))?;
                if existing_event != event {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "idempotent_campaign_export_conflict",
                    ));
                }
                return self
                    .commit_event(
                        metadata,
                        &request.campaign_id,
                        &request.export_id,
                        ("campaign_export", "campaign_export.request"),
                        &event,
                        vec![projection_target(
                            "public.campaign_exports",
                            &request.export_id,
                        )],
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_export_identity_conflict",
            ));
        }

        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.export_id,
                ("campaign_export", "campaign_export.request"),
                &event,
                vec![projection_target(
                    "public.campaign_exports",
                    &request.export_id,
                )],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_campaign_export_request")
            .await?;
        sqlx::query(
            r#"
            INSERT INTO public.campaign_exports (
                export_id, campaign_id, requested_by, audience, state,
                requested_at, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, 'REQUESTED', $5, 1,
                $6, $7, $8, $9, $10, $11
            )
            "#,
        )
        .bind(&request.export_id)
        .bind(&request.campaign_id)
        .bind(&request.requested_by)
        .bind(&request.audience)
        .bind(requested_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_export"))?;
        sqlx::query(
            r#"
            INSERT INTO public.campaign_export_jobs (
                export_id, campaign_id, state, requested_event_sequence
            ) VALUES ($1, $2, 'REQUESTED', $3)
            "#,
        )
        .bind(&request.export_id)
        .bind(&request.campaign_id)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_export_job"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_campaign_export_request"))?;
        Ok(persisted)
    }

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

fn campaign_projection_from_row(row: sqlx::postgres::PgRow) -> CampaignProjection {
    CampaignProjection {
        campaign_id: row.get("campaign_id"),
        owner_user_id: row.get("owner_user_id"),
        authority_contract_id: row.get("authority_contract_id"),
        title: row.get("title"),
        state: row.get("state"),
        aggregate_version: row.get("version"),
        last_event_sequence: row.get("last_event_sequence"),
    }
}
