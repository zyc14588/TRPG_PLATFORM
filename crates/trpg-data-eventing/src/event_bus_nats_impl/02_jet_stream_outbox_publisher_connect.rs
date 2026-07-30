
impl JetStreamOutboxPublisher {
    pub async fn connect(
        canonical: PostgresCanonicalStore,
        nats_url: &str,
        worker_id: &str,
        nats_ca_certificate_path: Option<&Path>,
    ) -> Result<Self, JetStreamOutboxError> {
        Self::connect_with_credentials(
            canonical,
            nats_url,
            worker_id,
            nats_ca_certificate_path,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn connect_with_credentials(
        canonical: PostgresCanonicalStore,
        nats_url: &str,
        worker_id: &str,
        nats_ca_certificate_path: Option<&Path>,
        nats_client_certificate_path: Option<&Path>,
        nats_client_private_key_path: Option<&Path>,
        nats_credentials_path: Option<&Path>,
    ) -> Result<Self, JetStreamOutboxError> {
        validate_worker_id(worker_id)?;
        canonical
            .verify_integrity()
            .await
            .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
        let pool = canonical.primary_pool();

        let (local_nats, tls_nats) = validate_nats_url(nats_url)?;
        let url_credentials = nats_url_credentials(nats_url)?;
        let connection_url = nats_endpoint_without_userinfo(nats_url)?;
        if !local_nats && nats_credentials_path.is_none() && url_credentials.is_none() {
            return Err(JetStreamOutboxError::Configuration(
                "remote_nats_credentials_required",
            ));
        }
        if nats_credentials_path.is_some() && url_credentials.is_some() {
            return Err(JetStreamOutboxError::Configuration(
                "ambiguous_nats_credentials",
            ));
        }
        if nats_client_certificate_path.is_some() != nats_client_private_key_path.is_some() {
            return Err(JetStreamOutboxError::Configuration(
                "nats_client_certificate_and_key_required_together",
            ));
        }
        let mut options = ConnectOptions::new()
            .name(worker_id)
            .require_tls(tls_nats || !local_nats)
            .connection_timeout(Duration::from_secs(5));
        if tls_nats {
            options = options.tls_first();
        }
        if let Some(path) = nats_ca_certificate_path {
            options = options.add_root_certificates(path.to_path_buf());
        }
        if let (Some(certificate), Some(private_key)) =
            (nats_client_certificate_path, nats_client_private_key_path)
        {
            options = options
                .add_client_certificate(certificate.to_path_buf(), private_key.to_path_buf());
        }
        if let Some((username, password)) = url_credentials {
            options = options.user_and_password(username, password);
        } else if let Some(path) = nats_credentials_path {
            options = options
                .credentials_file(path)
                .await
                .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_credentials"))?;
        }
        let client = options
            .connect(connection_url)
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?;
        let repository = PostgresOutboxLeaseRepository::new(
            pool.clone(),
            worker_id,
            OutboxLeasePolicy::default(),
        )
        .map_err(map_worker_error)?;
        let projection =
            PostgresProjectionWorker::new(pool.clone(), "canonical_event_projection", 250)
                .map_err(map_worker_error)?;
        let rag = PostgresRagSnapshotRepository::new(pool);
        Ok(Self {
            canonical,
            repository,
            projection,
            rag,
            client: client.clone(),
            jetstream: async_nats::jetstream::new(client),
            metrics: Arc::new(EventingMetrics::default()),
            batch_size: 100,
        })
    }

    /// Subscribes only to canonical outbox subjects and exposes a payload-free
    /// wake-up stream. Consumers must reload and authorize Event Store rows;
    /// NATS is never treated as replay truth.
    pub async fn subscribe_canonical_notifications(
        &self,
    ) -> Result<CanonicalNotificationSubscription, JetStreamOutboxError> {
        let subscriber = self
            .client
            .subscribe("trpg.events.>")
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?;
        Ok(CanonicalNotificationSubscription { subscriber })
    }

    pub fn with_metrics(mut self, metrics: Arc<EventingMetrics>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> Arc<EventingMetrics> {
        Arc::clone(&self.metrics)
    }

    pub async fn ensure_stream(&self) -> Result<(), JetStreamOutboxError> {
        let desired = canonical_stream_config();
        let mut stream = self
            .jetstream
            .get_or_create_stream(desired.clone())
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?;
        let info = stream
            .info()
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?;
        if !stream_config_matches(&info.config, &desired) {
            return Err(JetStreamOutboxError::Configuration(
                "jetstream_stream_contract_mismatch",
            ));
        }
        Ok(())
    }

    pub async fn check_readiness(&self) -> Result<(), JetStreamOutboxError> {
        self.canonical
            .verify_integrity()
            .await
            .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
        self.ensure_stream().await?;
        self.projection
            .check_readiness()
            .await
            .map_err(map_worker_error)?;
        self.rag
            .check_readiness()
            .await
            .map_err(|_| JetStreamOutboxError::Database("rag_read_model_readiness"))
    }

    pub async fn rebuild_projections_to_tip(
        &self,
    ) -> Result<Vec<ProjectionCheckpointState>, JetStreamOutboxError> {
        self.canonical
            .verify_integrity()
            .await
            .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
        self.projection
            .rebuild_all_to_tip()
            .await
            .map_err(map_worker_error)
    }

    pub async fn publish_batch(&self) -> Result<PublishBatchResult, JetStreamOutboxError> {
        self.canonical
            .verify_integrity()
            .await
            .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
        let mut result = PublishBatchResult::default();
        self.repository
            .quarantine_unverified_history()
            .await
            .map_err(map_worker_error)?;
        let acknowledgement_budget = self
            .repository
            .lease_duration()
            .checked_div(2)
            .filter(|duration| !duration.is_zero())
            .ok_or(JetStreamOutboxError::Configuration(
                "outbox_lease_too_short_for_publish",
            ))?;
        // Claim immediately before each external publish. This keeps rows that
        // are later in the configured batch out of a ticking lease while an
        // earlier JetStream acknowledgement is pending.
        for _ in 0..self.batch_size {
            let mut claimed = self
                .repository
                .claim_batch(1)
                .await
                .map_err(map_worker_error)?;
            let Some(row) = claimed.pop() else {
                break;
            };
            self.canonical
                .verify_integrity()
                .await
                .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
            result.claimed += 1;
            let publish_result =
                tokio::time::timeout(acknowledgement_budget, self.publish_one(&row))
                    .await
                    .map_err(|_| JetStreamOutboxError::PublishAcknowledgementTimedOut)
                    .and_then(|result| result);
            match publish_result {
                Ok(()) => {
                    self.repository
                        .mark_published(&row)
                        .await
                        .map_err(map_worker_error)?;
                    self.metrics.record_outbox_publish(&row, "published");
                    result.published += 1;
                }
                Err(error) => {
                    let failure = match error {
                        JetStreamOutboxError::InvalidOutboxPayload => {
                            OutboxFailureCode::InvalidEnvelope
                        }
                        JetStreamOutboxError::PublishAcknowledgementTimedOut => {
                            OutboxFailureCode::PublishAcknowledgementTimedOut
                        }
                        _ => OutboxFailureCode::JetStreamPublishFailed,
                    };
                    let disposition = self
                        .repository
                        .mark_failed(&row, failure)
                        .await
                        .map_err(map_worker_error)?;
                    self.metrics.record_outbox_publish(&row, "failed");
                    result.failed += 1;
                    if disposition.dead_lettered {
                        result.dead_lettered += 1;
                    }
                }
            }
        }
        result.dead_letter_total = self
            .repository
            .dead_letter_count()
            .await
            .map_err(map_worker_error)?;
        Ok(result)
    }

    pub async fn pending_count(&self) -> Result<i64, JetStreamOutboxError> {
        self.repository
            .pending_count()
            .await
            .map_err(map_worker_error)
    }

    pub async fn stream_message_count(&self) -> Result<u64, JetStreamOutboxError> {
        let mut stream = self
            .jetstream
            .get_stream(STREAM_NAME)
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?;
        Ok(stream
            .info()
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?
            .state
            .messages)
    }

    #[tracing::instrument(
        name = "jetstream_outbox_publish",
        skip_all,
        fields(
            correlation_id = %row.correlation_id,
            causation_id = %row.causation_id,
            event_sequence = row.event_sequence,
            campaign_id = %row.campaign_id,
            visibility_label = %row.visibility_label,
            provenance_kind = %row.provenance_kind
        )
    )]
    async fn publish_one(&self, row: &OutboxClaim) -> Result<(), JetStreamOutboxError> {
        // Validate after claiming so one corrupt row follows the ordinary
        // per-row failure/dead-letter path without retaining every other
        // claim in the batch until the lease expires.
        row.validate_for_publish().map_err(map_worker_error)?;
        let envelope = serde_json::to_vec(&event_envelope(row)?)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?;
        let headers = outbox_headers(row)?;
        self.jetstream
            .publish_with_headers(canonical_delivery_subject(row), headers, envelope.into())
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?;
        Ok(())
    }
}

fn canonical_stream_config() -> StreamConfig {
    StreamConfig {
        name: STREAM_NAME.to_owned(),
        description: Some("Canonical TRPG event outbox".to_owned()),
        subjects: vec!["trpg.events.>".to_owned()],
        retention: RetentionPolicy::Limits,
        discard: DiscardPolicy::Old,
        max_bytes: 10 * 1024 * 1024 * 1024,
        max_messages: -1,
        max_messages_per_subject: -1,
        max_consumers: -1,
        max_age: Duration::from_secs(7 * 24 * 60 * 60),
        max_message_size: -1,
        duplicate_window: Duration::from_secs(120),
        storage: StorageType::File,
        num_replicas: 1,
        no_ack: false,
        // Exact data-subject messages may be removed by the privacy worker.
        // Whole-stream purge remains prohibited.
        deny_delete: false,
        deny_purge: true,
        // NATS 2.10 normalizes an omitted compression override to an explicit
        // `none`; make the canonical contract explicit so fail-closed
        // comparison does not mistake server normalization for drift.
        compression: Some(async_nats::jetstream::stream::Compression::None),
        ..Default::default()
    }
}
