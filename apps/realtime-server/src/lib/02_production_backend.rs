impl RealtimeBackend for ProductionRealtimeBackend {
    type Session = ProductionRealtimeSession;

    fn authenticate<'a>(
        &'a self,
        bearer_token: &'a str,
        campaign_id: &'a str,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<Self::Session, BackendError>> {
        let bearer_token = bearer_token.to_owned();
        let campaign_id = campaign_id.to_owned();
        let ordinal = self.connection_sequence.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            let identity = self
                .identity
                .authenticate(Some(&bearer_token), &campaign_id, now_unix_ms)
                .await
                .map_err(map_identity_error)?;
            Ok(ProductionRealtimeSession {
                identity,
                connection_id: format!(
                    "realtime_{}_{}_{}",
                    std::process::id(),
                    now_unix_ms,
                    ordinal
                ),
            })
        })
    }

    fn binding(&self, session: &Self::Session) -> ConnectionBinding {
        self.protocol_binding(session)
    }

    fn authorize_subscription<'a>(
        &'a self,
        session: &'a mut Self::Session,
        subscription: &'a RoomSubscription,
        cursor: u64,
        resume_token: Option<&'a str>,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<(), BackendError>> {
        Box::pin(async move {
            validate_subscription(session.identity.binding(), subscription)?;
            self.reauthorize(session, subscription, now_unix_ms).await?;
            match (cursor, resume_token) {
                (0, None) => Ok(()),
                (_, Some(token)) => self
                    .resume_tokens
                    .verify(token, &self.resume_binding(session), cursor, now_unix_ms)
                    .map(|_| ())
                    .map_err(|_| BackendError::ResumeToken),
                _ => Err(BackendError::ResumeToken),
            }
        })
    }

    fn reauthorize<'a>(
        &'a self,
        session: &'a mut Self::Session,
        subscription: &'a RoomSubscription,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<ConnectionBinding, BackendError>> {
        Box::pin(async move {
            let previous_epoch = session.identity.binding().authority_epoch;
            let previous_campaign = session.identity.binding().campaign_id.clone();
            let refreshed = self
                .identity
                .reauthorize(&mut session.identity, now_unix_ms)
                .await
                .map_err(map_identity_error)?;
            if subscription.kind == RoomKind::Group
                && !session
                    .identity
                    .can_subscribe_private_group(&subscription.room_id, now_unix_ms)
                    .await
                    .map_err(map_identity_error)?
            {
                return Err(BackendError::Authorization);
            }
            if refreshed.campaign_id != previous_campaign {
                return Err(BackendError::Authorization);
            }
            if refreshed.authority_epoch != previous_epoch {
                return Err(BackendError::AuthorityChanged);
            }
            Ok(self.protocol_binding(session))
        })
    }

    fn replay<'a>(
        &'a self,
        session: &'a Self::Session,
        subscription: &'a RoomSubscription,
        cursor: u64,
        limit: usize,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<ReplayBatch, BackendError>> {
        Box::pin(async move {
            let binding = session.identity.binding();
            let bounds = self
                .canonical
                .replay_sequence_bounds(&binding.campaign_id)
                .await
                .map_err(|_| BackendError::Unavailable)?;
            let Some((earliest, latest)) = bounds else {
                return Ok(ReplayBatch {
                    events: Vec::new(),
                    source_cursor: cursor,
                    latest_cursor: cursor,
                    has_more: false,
                });
            };
            if (cursor > 0 && cursor.saturating_add(1) < earliest) || cursor > latest {
                return Err(BackendError::Resync(ResyncRequired {
                    reason: if cursor > latest {
                        "cursor_ahead_of_canonical_tip"
                    } else {
                        "cursor_older_than_retained_history"
                    },
                    earliest_cursor: earliest.saturating_sub(1),
                    latest_cursor: latest,
                }));
            }
            let after = i64::try_from(cursor).map_err(|_| BackendError::InvalidData)?;
            let limit = i64::try_from(limit.min(500)).map_err(|_| BackendError::InvalidData)?;
            let canonical = self
                .canonical
                .load_replay_page(&binding.campaign_id, after, limit)
                .await
                .map_err(|_| BackendError::Unavailable)?;
            let mut source_cursor = cursor;
            let mut events = Vec::with_capacity(canonical.len());
            for record in canonical {
                source_cursor =
                    u64::try_from(record.sequence).map_err(|_| BackendError::InvalidData)?;
                if u64::try_from(record.authority_contract_version)
                    .map_err(|_| BackendError::InvalidData)?
                    != binding.authority_epoch
                    || record.authority_mode.to_ascii_lowercase() != binding.authority_mode
                {
                    return Err(BackendError::AuthorityChanged);
                }
                let visibility = Visibility::try_from_parts(
                    &record.visibility_label,
                    nonempty(&record.visibility_subject),
                )
                .map_err(|_| BackendError::InvalidData)?;
                let campaign_id =
                    EntityId::new(&record.campaign_id).map_err(|_| BackendError::InvalidData)?;
                if !session
                    .identity
                    .can_view(&campaign_id, &visibility, now_unix_ms)
                    .await
                    .map_err(map_identity_error)?
                {
                    continue;
                }
                let event = realtime_event(record)?;
                if subscription.permits(&event) {
                    events.push(event);
                }
            }
            Ok(ReplayBatch {
                events,
                source_cursor,
                latest_cursor: latest,
                has_more: source_cursor < latest,
            })
        })
    }

    fn issue_resume_token(
        &self,
        session: &Self::Session,
        cursor: u64,
        now_unix_ms: u64,
    ) -> Result<String, BackendError> {
        let expires_at = now_unix_ms
            .checked_add(
                u64::try_from(self.resume_ttl.as_millis())
                    .map_err(|_| BackendError::InvalidData)?,
            )
            .ok_or(BackendError::InvalidData)?;
        self.resume_tokens
            .issue(
                &self.resume_binding(session),
                cursor,
                now_unix_ms,
                expires_at,
            )
            .map_err(|_| BackendError::Unavailable)
    }

    fn check_readiness(&self) -> RealtimeFuture<'_, Result<(), BackendError>> {
        Box::pin(async move {
            self.identity
                .check_readiness()
                .await
                .map_err(|_| BackendError::Unavailable)?;
            self.canonical
                .verify_integrity()
                .await
                .map_err(|_| BackendError::Unavailable)?;
            self.jetstream
                .check_readiness()
                .await
                .map_err(|_| BackendError::Unavailable)?;
            let mut cache = self.cache.clone();
            cache
                .check_readiness()
                .await
                .map_err(|_| BackendError::Unavailable)
        })
    }
}
