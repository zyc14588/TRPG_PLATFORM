
impl PostgresCanonicalStore {
    pub async fn connect(
        primary_url: &str,
        witness_url: &str,
        integrity_key_id: impl Into<String>,
        integrity_key: &[u8],
        payload_key_reference: impl Into<String>,
        payload_key: &[u8],
    ) -> Result<Self, CanonicalStoreError> {
        if integrity_key.len() != 32 {
            return Err(CanonicalStoreError::Configuration(
                "32_byte_integrity_key_required",
            ));
        }
        if payload_key.len() != 32 {
            return Err(CanonicalStoreError::Configuration(
                "32_byte_payload_encryption_key_required",
            ));
        }
        if payload_key == integrity_key {
            return Err(CanonicalStoreError::Configuration(
                "payload_and_integrity_keys_must_be_distinct",
            ));
        }
        let integrity_key_id = integrity_key_id.into();
        if integrity_key_id.trim().is_empty() {
            return Err(CanonicalStoreError::Configuration(
                "integrity_key_id_required",
            ));
        }

        let primary_options = parse_connection_options(primary_url, "primary")?;
        let witness_options = parse_connection_options(witness_url, "witness")?;
        if same_endpoint(&primary_options, &witness_options) {
            return Err(CanonicalStoreError::Configuration(
                "independent_witness_endpoint_required",
            ));
        }

        let primary = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(primary_options)
            .await
            .map_err(|_| CanonicalStoreError::Connection {
                component: "primary",
            })?;
        let witness = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(witness_options)
            .await
            .map_err(|_| CanonicalStoreError::Connection {
                component: "witness",
            })?;

        let mut key = Zeroizing::new([0_u8; 32]);
        key.copy_from_slice(integrity_key);
        let payload_cipher = PayloadCipher::new(payload_key_reference, payload_key)
            .map_err(|_| CanonicalStoreError::Configuration("payload_cipher_invalid"))?;
        Ok(Self {
            primary,
            witness,
            integrity_key_id,
            integrity_key: Arc::new(key),
            payload_cipher: Arc::new(payload_cipher),
        })
    }

    fn integrity_key(&self) -> &[u8; 32] {
        &self.integrity_key
    }

    /// Gives trusted composition adapters a clone of the primary pool. The
    /// canonical store remains the integrity authority and must be retained by
    /// every adapter that consumes this pool.
    pub fn primary_pool(&self) -> PgPool {
        self.primary.clone()
    }

    /// Derives a retry-stable, non-persisted invitation bearer token from the
    /// canonical integrity secret with an explicit domain separator. Only the
    /// P06 repository can call this crate-private helper; logs and Debug output
    /// never receive the result.
    pub(crate) fn derive_campaign_invite_token(
        &self,
        campaign_id: &str,
        invite_id: &str,
        invited_user_id: &str,
        role: &str,
        expires_at_unix_ms: u64,
        idempotency_key: &str,
    ) -> Result<String, CanonicalStoreError> {
        let fields = [
            "campaign_invite_token_v1",
            campaign_id,
            invite_id,
            invited_user_id,
            role,
            idempotency_key,
        ];
        if fields.iter().any(|field| field.trim().is_empty()) {
            return Err(CanonicalStoreError::Validation(
                "invite_token_binding_invalid",
            ));
        }
        let sealed = hmac_fields(
            self.integrity_key(),
            &[
                fields[0].to_owned(),
                fields[1].to_owned(),
                fields[2].to_owned(),
                fields[3].to_owned(),
                fields[4].to_owned(),
                expires_at_unix_ms.to_string(),
                fields[5].to_owned(),
            ],
        );
        sealed
            .strip_prefix("hmac-sha256:")
            .map(str::to_owned)
            .ok_or(CanonicalStoreError::IntegrityViolation(
                "invite_token_derivation_invalid",
            ))
    }

    /// Derives the one-time projection capability for a canonical commit. Only
    /// the trusted repository process receives the preimage; Event Store keeps
    /// a SHA-256 verifier inside the HMAC-protected target list. A database role
    /// that can read the event and write a projection therefore still cannot
    /// consume the target without the canonical integrity secret.
    pub(crate) fn derive_core_projection_capability(
        &self,
        commit_id: &str,
    ) -> Result<Zeroizing<String>, CanonicalStoreError> {
        if commit_id.trim().is_empty() || commit_id.len() > 160 {
            return Err(CanonicalStoreError::Validation(
                "projection_capability_binding_invalid",
            ));
        }
        let sealed = hmac_fields(
            self.integrity_key(),
            &[
                "core_projection_capability_v1".to_owned(),
                commit_id.to_owned(),
            ],
        );
        let capability =
            sealed
                .strip_prefix("hmac-sha256:")
                .ok_or(CanonicalStoreError::IntegrityViolation(
                    "projection_capability_derivation_invalid",
                ))?;
        Ok(Zeroizing::new(capability.to_owned()))
    }

    pub async fn apply_migrations(&self) -> Result<(), CanonicalStoreError> {
        crate::persistence_migrations::migrator()
            .run(&self.primary)
            .await
            .map_err(|error| match error {
                sqlx::migrate::MigrateError::VersionMismatch(version) => {
                    CanonicalStoreError::MigrationChecksumMismatch {
                        component: "primary",
                        version,
                    }
                }
                _ => CanonicalStoreError::Migration {
                    component: "primary",
                },
            })?;

        crate::persistence_migrations::witness_migrator()
            .run(&self.witness)
            .await
            .map_err(|error| match error {
                sqlx::migrate::MigrateError::VersionMismatch(version) => {
                    CanonicalStoreError::MigrationChecksumMismatch {
                        component: "witness",
                        version,
                    }
                }
                _ => CanonicalStoreError::Migration {
                    component: "witness",
                },
            })?;
        Ok(())
    }

    /// Apply migrations, reconcile crash gaps, and prove both chains agree.
    /// Production composition should call this before accepting traffic.
    pub async fn prepare_for_service(&self) -> Result<RecoveryReport, CanonicalStoreError> {
        self.apply_migrations().await?;
        self.recover().await
    }

    #[tracing::instrument(
        name = "canonical_commit",
        skip_all,
        fields(
            correlation_id = %draft.correlation_id,
            causation_id = %draft.causation_id,
            commit_id = %draft.commit_id,
            campaign_id = %draft.campaign_id,
            stream_id = %draft.stream_id
        )
    )]
    pub async fn commit(
        &self,
        draft: &AtomicCommitDraft,
    ) -> Result<PersistedCommit, CanonicalStoreError> {
        self.commit_with_projection(draft, None).await
    }

    /// P07 canonical commit path. The supplied projection is applied by a
    /// narrowly granted SECURITY DEFINER function before the Event Store
    /// transaction commits. Its canonical JSON hash must already be present
    /// as an HMAC-bound projection target in the event draft.
    pub(crate) async fn commit_player_action_projection(
        &self,
        draft: &AtomicCommitDraft,
        projection: &serde_json::Value,
    ) -> Result<PersistedCommit, CanonicalStoreError> {
        self.commit_with_projection(draft, Some(AtomicProjection::PlayerAction(projection)))
            .await
    }

    /// Atomically appends CampaignInviteAccepted and creates the invited
    /// membership. A uniqueness or role conflict aborts the entire canonical
    /// transaction, so an invite can never be consumed without its projection.
    pub(crate) async fn commit_campaign_invite_acceptance(
        &self,
        draft: &AtomicCommitDraft,
        projection: &serde_json::Value,
    ) -> Result<PersistedCommit, CanonicalStoreError> {
        self.commit_with_projection(
            draft,
            Some(AtomicProjection::CampaignInviteAcceptance(projection)),
        )
        .await
    }

    /// Atomically appends one P08 gameplay event and permanently reserves
    /// every opaque server roll consumed by that event. The remaining state
    /// projections stay rebuildable, but a cancellation or projection error
    /// after this transaction can no longer make the same roll reusable.
    pub(crate) async fn commit_gameplay_roll_reservation(
        &self,
        draft: &AtomicCommitDraft,
        projection: &serde_json::Value,
    ) -> Result<PersistedCommit, CanonicalStoreError> {
        self.commit_with_projection(
            draft,
            Some(AtomicProjection::GameplayRollReservation(projection)),
        )
        .await
    }

    /// Atomically appends one EndingRecorded event and reserves the Session's
    /// single canonical ending. The ending read model remains independently
    /// rebuildable, while a projector crash can no longer make the Session
    /// available to a different ending command.
    pub(crate) async fn commit_session_ending_reservation(
        &self,
        draft: &AtomicCommitDraft,
        projection: &serde_json::Value,
    ) -> Result<PersistedCommit, CanonicalStoreError> {
        self.commit_with_projection(
            draft,
            Some(AtomicProjection::SessionEndingReservation(projection)),
        )
        .await
    }

    /// Atomically appends the externally authorized AgentJobRequested event
    /// and materializes the durable workflow row that workers claim. The
    /// projection is content-addressed in the event's HMAC-bound target list,
    /// so a canonical event can never be committed without its exact job
    /// binding and a conflicting durable row aborts the Event Store append.
    pub async fn commit_agent_job_request(
        &self,
        request: &CanonicalCommitRequest,
        projection: &serde_json::Value,
    ) -> KernelResult<CanonicalCommitReceipt> {
        let mut draft = canonical_request_draft(request)?;
        if draft.events.len() != 1
            || draft.events[0].event_type != "AgentJobRequested"
            || serde_json::from_str::<serde_json::Value>(&draft.events[0].payload_json).ok()
                != Some(projection.clone())
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        let projection_id: String = sqlx::query_scalar(
            "SELECT core_domain.agent_job_request_projection_id($1::JSONB)",
        )
        .bind(Json(projection.clone()))
        .fetch_one(&self.primary)
        .await
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        draft.events[0].projection_targets = vec![CanonicalProjectionTarget {
            relation: "core_domain.agent_job_request".to_owned(),
            row_id: projection_id,
        }];

        let persisted = self
            .commit_with_projection(&draft, Some(AtomicProjection::AgentJobRequest(projection)))
            .await
            .map_err(map_canonical_port_error)?;
        self.verify_integrity()
            .await
            .map_err(map_canonical_port_error)?;
        let events = load_committed_events(&self.primary, &self.payload_cipher, &persisted)
            .await
            .map_err(map_canonical_port_error)?;
        Ok(CanonicalCommitReceipt {
            first_stream_version: u64::try_from(persisted.first_stream_version)
                .map_err(|_| TrpgError::AuditIntegrityViolation)?,
            last_stream_version: u64::try_from(persisted.last_stream_version)
                .map_err(|_| TrpgError::AuditIntegrityViolation)?,
            events,
        })
    }

    async fn commit_with_projection(
        &self,
        draft: &AtomicCommitDraft,
        atomic_projection: Option<AtomicProjection<'_>>,
    ) -> Result<PersistedCommit, CanonicalStoreError> {
        let normalized = normalize_and_validate(draft)?;
        self.verify_cryptographic_chains().await?;
        let request_hash = request_hash(&normalized);

        if let Some(existing) = self
            .load_existing_commit(
                &normalized.commit_id,
                &normalized.campaign_id,
                &normalized.stream_id,
                &normalized.idempotency_key,
            )
            .await?
        {
            let stored_request_hash = self
                .validate_existing_commit(&existing, &normalized, &request_hash)
                .await?;
            self.finalize_witness(&existing, &stored_request_hash)
                .await?;
            return Ok(existing);
        }

        let prepared = self
            .append_witness(
                &normalized.commit_id,
                WitnessPhase::Prepared,
                &request_hash,
                None,
                None,
                "primary_commit_pending",
            )
            .await?;

        let (persisted, persisted_request_hash) = match self
            .commit_primary(&normalized, &request_hash, &prepared, atomic_projection)
            .await
        {
            Ok(persisted) => persisted,
            Err(error) => return Err(error),
        };

        self.finalize_witness(&persisted, &persisted_request_hash)
            .await?;
        Ok(persisted)
    }
}
