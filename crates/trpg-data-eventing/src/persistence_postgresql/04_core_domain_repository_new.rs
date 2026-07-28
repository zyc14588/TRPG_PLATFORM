
impl CoreDomainRepository {
    /// Composes the API-owned projection pool with the separately credentialed
    /// canonical store. Production must not reuse the canonical service role
    /// for identity or business-table reads/writes.
    pub fn new(projection_pool: PgPool, canonical: PostgresCanonicalStore) -> Self {
        Self::new_with_clock(projection_pool, canonical, Arc::new(SystemCoreDomainClock))
    }

    pub fn new_with_clock(
        projection_pool: PgPool,
        canonical: PostgresCanonicalStore,
        clock: Arc<dyn CoreDomainClock>,
    ) -> Self {
        Self {
            primary: projection_pool,
            canonical,
            clock,
        }
    }

    pub async fn connect(
        database_url: &str,
        canonical: PostgresCanonicalStore,
    ) -> Result<Self, CoreDomainRepositoryError> {
        let options = PgConnectOptions::from_str(database_url)
            .map_err(|_| CoreDomainRepositoryError::Database("parse_projection_database_url"))?;
        let primary = PgPoolOptions::new()
            .max_connections(20)
            .connect_with(options)
            .await
            .map_err(database_error("connect_projection_database"))?;
        Ok(Self::new(primary, canonical))
    }

    pub fn primary_pool(&self) -> PgPool {
        self.primary.clone()
    }

    pub async fn character_owner_user_id(
        &self,
        campaign_id: &str,
        character_id: &str,
    ) -> Result<String, CoreDomainRepositoryError> {
        let row = sqlx::query(
            "SELECT campaign_id, owner_user_id \
             FROM public.characters WHERE character_id = $1",
        )
        .bind(character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_owner"))?
        .ok_or(CoreDomainRepositoryError::NotFound("character"))?;
        if row.get::<String, _>("campaign_id") != campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        Ok(row.get("owner_user_id"))
    }

    async fn set_projection_capability(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        commit_id: &str,
        operation: &'static str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let capability = self
            .canonical
            .derive_core_projection_capability(commit_id)?;
        sqlx::query("SELECT set_config('trpg.projection_capability', $1, TRUE)")
            .bind(capability.as_str())
            .execute(&mut **transaction)
            .await
            .map_err(database_error(operation))?;
        Ok(())
    }

    async fn begin_projection_transaction<'a>(
        &'a self,
        commit_id: &str,
        operation: &'static str,
    ) -> Result<Transaction<'a, Postgres>, CoreDomainRepositoryError> {
        let mut transaction = self
            .primary
            .begin()
            .await
            .map_err(database_error(operation))?;
        self.set_projection_capability(&mut transaction, commit_id, "set_projection_capability")
            .await?;
        Ok(transaction)
    }

    async fn commit_event(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        stream_id: &str,
        route: (&str, &str),
        event: &CoreDomainEvent,
        projection_targets: Vec<CanonicalProjectionTarget>,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let draft = metadata.to_draft(
            campaign_id,
            stream_id,
            route.0,
            route.1,
            event,
            projection_targets,
        )?;
        self.canonical.commit(&draft).await.map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    async fn commit_gameplay_event(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        stream_id: &str,
        route: (&str, &str),
        event: &CoreDomainEvent,
        mut projection_targets: Vec<CanonicalProjectionTarget>,
        aggregate_kind: &str,
        consumptions: &[GameplayRollConsumption],
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if consumptions.is_empty() {
            return self
                .commit_event(
                    metadata,
                    campaign_id,
                    stream_id,
                    route,
                    event,
                    projection_targets,
                )
                .await;
        }
        let existing_uses_reservation: Option<bool> = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
                  FROM public.event_store AS event
                  CROSS JOIN LATERAL jsonb_array_elements(
                      event.projection_targets
                  ) AS target
                 WHERE event.sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
                   AND target ->> 'relation' =
                       'core_domain.gameplay_roll_reservation'
            )
              FROM public.formal_commits AS formal
             WHERE formal.commit_id = $1
               AND formal.status = 'committed'
            "#,
        )
        .bind(&metadata.commit_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error(
            "load_gameplay_roll_reservation_commit_shape",
        ))?;
        if existing_uses_reservation == Some(false) {
            // Events committed before the atomic-reservation migration have
            // an immutable request hash without the new HMAC-bound marker.
            // Preserve their exact retry shape; their already materialized
            // consumption row remains the durable ownership record.
            return self
                .commit_event(
                    metadata,
                    campaign_id,
                    stream_id,
                    route,
                    event,
                    projection_targets,
                )
                .await;
        }
        let projection = serde_json::json!({
            "campaign_id": campaign_id,
            "aggregate_kind": aggregate_kind,
            "aggregate_id": stream_id,
            "visibility_label": metadata.visibility_label,
            "visibility_subject": metadata.visibility_subject,
            "provenance_kind": metadata.provenance_kind,
            "provenance_reference": metadata.provenance_reference,
            "provenance_recorded_by": metadata.provenance_recorded_by,
            "consumptions": consumptions
                .iter()
                .map(|consumption| serde_json::json!({
                    "roll_id": consumption.roll_id,
                    "roll_kind": consumption.roll_kind,
                }))
                .collect::<Vec<_>>(),
        });
        let projection_id = self
            .gameplay_roll_reservation_projection_id(&projection)
            .await?;
        projection_targets.push(projection_target(
            "core_domain.gameplay_roll_reservation",
            &projection_id,
        ));
        let draft = metadata.to_draft(
            campaign_id,
            stream_id,
            route.0,
            route.1,
            event,
            projection_targets,
        )?;
        self.canonical
            .commit_gameplay_roll_reservation(&draft, &projection)
            .await
            .map_err(Into::into)
    }

    async fn commit_ending_event(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordEndingRequest,
        event: &CoreDomainEvent,
        reservation: &Value,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let existing_uses_reservation: Option<bool> = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
                  FROM public.event_store AS event
                  CROSS JOIN LATERAL jsonb_array_elements(
                      event.projection_targets
                  ) AS target
                 WHERE event.sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
                   AND target ->> 'relation' =
                       'core_domain.session_ending_reservation'
            )
              FROM public.formal_commits AS formal
             WHERE formal.commit_id = $1
               AND formal.status = 'committed'
            "#,
        )
        .bind(&metadata.commit_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error(
            "load_session_ending_reservation_commit_shape",
        ))?;
        let mut targets = vec![projection_target(
            "public.ending_events",
            &request.ending_event_id,
        )];
        if existing_uses_reservation == Some(false) {
            // Preserve the immutable request shape for endings committed
            // before the canonical Session reservation marker existed.
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.ending_event_id,
                    ("ending", "ending.record"),
                    event,
                    targets,
                )
                .await;
        }
        let reservation_id = self
            .session_ending_reservation_projection_id(reservation)
            .await?;
        targets.push(projection_target(
            SESSION_ENDING_RESERVATION_RELATION,
            &reservation_id,
        ));
        let draft = metadata.to_draft(
            &request.campaign_id,
            &request.ending_event_id,
            "ending",
            "ending.record",
            event,
            targets,
        )?;
        self.canonical
            .commit_session_ending_reservation(&draft, reservation)
            .await
            .map_err(Into::into)
    }
}

fn player_action_event(
    event_type: &str,
    payload: serde_json::Value,
    projection_targets: Vec<CanonicalProjectionTarget>,
) -> Result<CanonicalEventDraft, CoreDomainRepositoryError> {
    Ok(CanonicalEventDraft {
        event_type: event_type.to_owned(),
        payload_json: serde_json::to_string(&payload)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?,
        visibility: None,
        projection_targets,
    })
}

fn canonical_success_level(
    roll: u8,
    target: u8,
) -> Result<&'static str, CoreDomainRepositoryError> {
    if !(1..=100).contains(&roll) || target > 100 {
        return Err(CoreDomainRepositoryError::InvalidInput("dice_range"));
    }
    Ok(if roll == 1 {
        "CRITICAL"
    } else if (target < 50 && roll >= 96) || (target >= 50 && roll == 100) {
        "FUMBLE"
    } else if roll <= target / 5 {
        "EXTREME"
    } else if roll <= target / 2 {
        "HARD"
    } else if roll <= target {
        "REGULAR"
    } else {
        "FAILURE"
    })
}

fn validate_server_dice_record(
    dice: &PlayerActionDiceRecord,
    expected_adjustment: &str,
) -> Result<(), CoreDomainRepositoryError> {
    if dice.selected_tens_digit > 9 || dice.ones_digit > 9 {
        return Err(CoreDomainRepositoryError::InvalidInput(
            "server_dice_record",
        ));
    }
    let reconstructed = if dice.selected_tens_digit == 0 && dice.ones_digit == 0 {
        100
    } else {
        dice.selected_tens_digit * 10 + dice.ones_digit
    };
    if EntityId::new(&dice.roll_id).is_err()
        || dice.target_value == 0
        || reconstructed != dice.rolled_value
        || dice.adjustment != expected_adjustment
        || canonical_success_level(dice.rolled_value, dice.target_value)? != dice.success_level
    {
        return Err(CoreDomainRepositoryError::InvalidInput(
            "server_dice_record",
        ));
    }
    Ok(())
}
