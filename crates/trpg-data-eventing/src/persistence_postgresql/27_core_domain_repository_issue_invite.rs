
impl CoreDomainRepository {

    pub async fn issue_invite(
        &self,
        metadata: &CoreCommandMetadata,
        request: &IssueInviteRequest,
    ) -> Result<IssuedCampaignInvite, CoreDomainRepositoryError> {
        if metadata.expected_version != 0 {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "invite_expected_version",
            ));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        self.ensure_user_exists(&request.invited_user_id).await?;

        let raw_token = self
            .canonical
            .derive_campaign_invite_token(
                &request.campaign_id,
                &request.invite_id,
                &request.invited_user_id,
                request.role.as_database_role(),
                request.expires_at_unix_ms,
                &metadata.idempotency_key,
            )
            .map_err(CoreDomainRepositoryError::Canonical)?;
        let token_digest = format!("sha256:{:x}", Sha256::digest(raw_token.as_bytes()));
        let invite = CampaignInvite::new(
            &request.invite_id,
            &request.campaign_id,
            &request.invited_user_id,
            &metadata.requesting_actor_id,
            request.role,
            &token_digest,
            request.expires_at_unix_ms,
            self.clock.now_unix_ms()?,
        )?;
        let event = CoreDomainEvent::CampaignInviteIssued {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            invite_id: invite.invite_id.to_string(),
            campaign_id: invite.campaign_id.to_string(),
            invited_user_id: invite.invited_user_id.to_string(),
            issued_by: invite.issued_by.to_string(),
            role: invite.role,
            token_digest,
            expires_at_unix_ms: invite.expires_at_unix_ms,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.invite_id,
                ("campaign_invite", "campaign.invite.issue"),
                &event,
                Vec::new(),
            )
            .await?;
        Ok(IssuedCampaignInvite {
            invite_id: request.invite_id.clone(),
            raw_token,
            expires_at_unix_ms: request.expires_at_unix_ms,
            persisted,
        })
    }

    async fn load_campaign_events(
        &self,
        campaign_id: &str,
    ) -> Result<Vec<CanonicalReplayEvent>, CoreDomainRepositoryError> {
        let mut events = Vec::new();
        let mut after_sequence = 0_i64;
        loop {
            let page = self
                .canonical
                .load_replay_page(campaign_id, after_sequence, 500)
                .await?;
            let page_len = page.len();
            if let Some(last) = page.last() {
                after_sequence = last.sequence;
            }
            events.extend(page);
            if page_len < 500 {
                break;
            }
        }
        Ok(events)
    }

    async fn load_idempotent_core_event_record(
        &self,
        campaign_id: &str,
        stream_id: &str,
        metadata: &CoreCommandMetadata,
        expected_event_type: &str,
    ) -> Result<Option<(i64, CoreDomainEvent)>, CoreDomainRepositoryError> {
        let mut matched = None;
        for replay in self.load_campaign_events(campaign_id).await? {
            if replay.stream_id != stream_id
                || replay.command_id != metadata.command_id
                || !canonical_event_idempotency_matches(
                    &replay.idempotency_key,
                    &metadata.idempotency_key,
                )
            {
                continue;
            }
            if replay.event_type != expected_event_type || matched.is_some() {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_event_binding_conflict",
                ));
            }
            matched = Some((
                replay.sequence,
                serde_json::from_value(replay.payload).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("idempotent_event_payload")
                })?,
            ));
        }
        Ok(matched)
    }

    async fn load_idempotent_core_event(
        &self,
        campaign_id: &str,
        stream_id: &str,
        metadata: &CoreCommandMetadata,
        expected_event_type: &str,
    ) -> Result<Option<CoreDomainEvent>, CoreDomainRepositoryError> {
        Ok(self
            .load_idempotent_core_event_record(
                campaign_id,
                stream_id,
                metadata,
                expected_event_type,
            )
            .await?
            .map(|(_, event)| event))
    }

    async fn load_gameplay_retry_projection_targets(
        &self,
        event_sequence: i64,
    ) -> Result<Vec<CanonicalProjectionTarget>, CoreDomainRepositoryError> {
        let stored: Value = sqlx::query_scalar(
            "SELECT projection_targets FROM public.event_store WHERE sequence = $1",
        )
        .bind(event_sequence)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_gameplay_retry_projection_targets"))?
        .ok_or(CoreDomainRepositoryError::Integrity(
            "gameplay_retry_event_missing",
        ))?;
        let targets = stored
            .as_array()
            .ok_or(CoreDomainRepositoryError::Integrity(
                "gameplay_retry_projection_targets",
            ))?;
        let mut parsed = Vec::new();
        let mut unique = BTreeSet::new();
        for target in targets {
            let relation = target.get("relation").and_then(Value::as_str).ok_or(
                CoreDomainRepositoryError::Integrity("gameplay_retry_projection_targets"),
            )?;
            let row_id = target.get("row_id").and_then(Value::as_str).ok_or(
                CoreDomainRepositoryError::Integrity("gameplay_retry_projection_targets"),
            )?;
            if relation == "core_domain.gameplay_roll_reservation" {
                continue;
            }
            if !unique.insert((relation.to_owned(), row_id.to_owned())) {
                return Err(CoreDomainRepositoryError::Integrity(
                    "gameplay_retry_projection_targets",
                ));
            }
            parsed.push(projection_target(relation, row_id));
        }
        Ok(parsed)
    }

    pub async fn accept_invite(
        &self,
        metadata: &CoreCommandMetadata,
        request: &AcceptInviteRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 1
            || metadata.requesting_actor_id != request.accepting_user_id
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "invite_accept_metadata",
            ));
        }
        let accepting_user = UserId::new(&request.accepting_user_id)?;
        let mut issued = None;
        let mut prior_acceptance = None;
        for replay in self.load_campaign_events(&request.campaign_id).await? {
            if replay.event_type == "CampaignInviteIssued" {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload)
                    .map_err(|_| CoreDomainRepositoryError::Integrity("invite_event_payload"))?;
                if let CoreDomainEvent::CampaignInviteIssued {
                    invite_id,
                    campaign_id,
                    invited_user_id,
                    issued_by,
                    role,
                    token_digest,
                    expires_at_unix_ms,
                    ..
                } = event
                {
                    if invite_id == request.invite_id {
                        issued = Some(CampaignInvite::new(
                            invite_id,
                            campaign_id,
                            invited_user_id,
                            issued_by,
                            role,
                            token_digest,
                            expires_at_unix_ms,
                            expires_at_unix_ms.saturating_sub(1),
                        )?);
                    }
                }
            } else if replay.event_type == "CampaignInviteAccepted" {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload)
                    .map_err(|_| CoreDomainRepositoryError::Integrity("invite_event_payload"))?;
                if let CoreDomainEvent::CampaignInviteAccepted {
                    invite_id,
                    campaign_id,
                    user_id,
                    role,
                    accepted_at_unix_ms,
                    ..
                } = event
                {
                    if invite_id == request.invite_id {
                        if user_id != request.accepting_user_id
                            || !canonical_event_idempotency_matches(
                                &replay.idempotency_key,
                                &metadata.idempotency_key,
                            )
                            || prior_acceptance.is_some()
                        {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "invite_already_consumed",
                            ));
                        }
                        prior_acceptance = Some((campaign_id, role, accepted_at_unix_ms));
                    }
                }
            }
        }
        let issued = issued.ok_or(CoreDomainRepositoryError::NotFound("campaign_invite"))?;
        let accepted_at_unix_ms = match prior_acceptance {
            Some((campaign_id, role, accepted_at_unix_ms))
                if campaign_id == request.campaign_id && role == issued.role =>
            {
                accepted_at_unix_ms
            }
            Some(_) => {
                return Err(CoreDomainRepositoryError::Integrity(
                    "invite_acceptance_binding_conflict",
                ));
            }
            None => self.clock.now_unix_ms()?,
        };
        issued.validate_acceptance(&accepting_user, accepted_at_unix_ms)?;
        let supplied_digest = format!("sha256:{:x}", Sha256::digest(request.raw_token.as_bytes()));
        if !token_digest_matches(&supplied_digest, &issued.token_digest) {
            return Err(CoreDomainRepositoryError::Forbidden);
        }

        let event = CoreDomainEvent::CampaignInviteAccepted {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            invite_id: request.invite_id.clone(),
            campaign_id: request.campaign_id.clone(),
            user_id: request.accepting_user_id.clone(),
            role: issued.role,
            accepted_at_unix_ms,
        };
        let accepted_at = timestamp_from_unix_ms(accepted_at_unix_ms, "invite.accepted_at")?;
        let projection = serde_json::json!({
            "kind": "ACCEPT",
            "invite_id": request.invite_id,
            "campaign_id": request.campaign_id,
            "user_id": request.accepting_user_id,
            "role": issued.role.as_database_role(),
            "granted_by": issued.issued_by.to_string(),
            "granted_at_unix_ms": accepted_at_unix_ms,
        });
        let projection_id = self
            .campaign_invite_acceptance_projection_id(&projection)
            .await?;
        let draft = metadata.to_draft(
            &request.campaign_id,
            &request.invite_id,
            "campaign_invite",
            "campaign.invite.accept",
            &event,
            vec![projection_target(
                "core_domain.campaign_invite_acceptance",
                &projection_id,
            )],
        )?;
        let persisted = self
            .canonical
            .commit_campaign_invite_acceptance(&draft, &projection)
            .await?;
        let membership = sqlx::query(
            r#"
            SELECT role, granted_at, revoked_at IS NULL AS active
              FROM public.campaign_memberships
             WHERE campaign_id = $1 AND user_id = $2
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.accepting_user_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("verify_invited_membership"))?
        .ok_or(CoreDomainRepositoryError::Integrity(
            "membership_projection_missing",
        ))?;
        if membership.get::<String, _>("role") != issued.role.as_database_role()
            || !membership.get::<bool, _>("active")
            || membership.get::<DateTime<Utc>, _>("granted_at") != accepted_at
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "membership_role_conflict",
            ));
        }
        Ok(persisted)
    }
}
