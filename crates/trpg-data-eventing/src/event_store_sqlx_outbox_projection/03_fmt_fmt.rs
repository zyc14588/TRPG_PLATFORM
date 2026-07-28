
impl fmt::Debug for PostgresCanonicalStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCanonicalStore")
            .field("primary", &"[POSTGRESQL POOL]")
            .field("witness", &"[INDEPENDENT POSTGRESQL POOL]")
            .field("integrity_key_id", &self.integrity_key_id)
            .field("integrity_key", &"[REDACTED]")
            .field("payload_cipher", &"[REDACTED]")
            .finish()
    }
}

/// Synchronous application port backed by the retained Tokio runtime that
/// owns the SQLx pools. Product composition roots keep this adapter private
/// and inject only the trait object into runtime/agent stores.
#[derive(Clone)]
pub struct PostgresCanonicalCommitPort {
    runtime: Arc<Mutex<tokio::runtime::Runtime>>,
    store: PostgresCanonicalStore,
}

impl PostgresCanonicalCommitPort {
    pub fn new(
        runtime: Arc<Mutex<tokio::runtime::Runtime>>,
        store: PostgresCanonicalStore,
    ) -> Self {
        Self { runtime, store }
    }

    fn commit_draft(&self, draft: AtomicCommitDraft) -> KernelResult<CanonicalCommitReceipt> {
        // The shared-kernel port is intentionally synchronous, but callers such
        // as the privacy workflow can already be running on a Tokio executor.
        // Tokio forbids entering a second Runtime from that executor thread.
        // Bridge only that nested case through a dedicated OS thread while the
        // retained SQLx Runtime remains the sole owner of its pools.
        if tokio::runtime::Handle::try_current().is_ok() {
            let runtime = Arc::clone(&self.runtime);
            let store = self.store.clone();
            return std::thread::Builder::new()
                .name("canonical-commit-bridge".to_owned())
                .spawn(move || {
                    let runtime = runtime
                        .lock()
                        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                    commit_on_runtime(&runtime, &store, &draft)
                })
                .map_err(|_| TrpgError::AuditIntegrityViolation)?
                .join()
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        }

        let runtime = self
            .runtime
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        commit_on_runtime(&runtime, &self.store, &draft)
    }
}

impl fmt::Debug for PostgresCanonicalCommitPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCanonicalCommitPort")
            .field("runtime", &"[RETAINED TOKIO RUNTIME]")
            .field("store", &self.store)
            .finish()
    }
}

impl CanonicalCommitPort for PostgresCanonicalCommitPort {
    fn load_receipt(
        &self,
        key: &CanonicalCommitKey,
    ) -> KernelResult<Option<CanonicalCommitReceipt>> {
        let runtime = Arc::clone(&self.runtime);
        let store = self.store.clone();
        let key = key.clone();
        let load = move || {
            let runtime = runtime
                .lock()
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
            load_receipt_on_runtime(&runtime, &store, &key)
        };
        if tokio::runtime::Handle::try_current().is_ok() {
            return std::thread::Builder::new()
                .name("canonical-receipt-lookup-bridge".to_owned())
                .spawn(load)
                .map_err(|_| TrpgError::AuditIntegrityViolation)?
                .join()
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        }
        load()
    }

    fn commit(&self, request: &CanonicalCommitRequest) -> KernelResult<CanonicalCommitReceipt> {
        let draft = canonical_request_draft(request)?;
        self.commit_draft(draft)
    }

    fn verify_receipt(
        &self,
        request: &CanonicalCommitRequest,
        receipt: &CanonicalCommitReceipt,
    ) -> KernelResult<()> {
        let draft = canonical_request_draft(request)?;
        let runtime = Arc::clone(&self.runtime);
        let store = self.store.clone();
        let receipt = receipt.clone();
        let verify = move || {
            let runtime = runtime
                .lock()
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
            runtime
                .block_on(verify_receipt_on_store(&store, &draft, &receipt))
                .map_err(map_canonical_port_error)
        };
        if tokio::runtime::Handle::try_current().is_ok() {
            return std::thread::Builder::new()
                .name("canonical-receipt-verification-bridge".to_owned())
                .spawn(verify)
                .map_err(|_| TrpgError::AuditIntegrityViolation)?
                .join()
                .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        }
        verify()
    }
}

fn load_receipt_on_runtime(
    runtime: &tokio::runtime::Runtime,
    store: &PostgresCanonicalStore,
    key: &CanonicalCommitKey,
) -> KernelResult<Option<CanonicalCommitReceipt>> {
    if key.commit_id.trim().is_empty()
        || key.campaign_id.trim().is_empty()
        || key.stream_id.trim().is_empty()
        || key.idempotency_key.trim().is_empty()
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    runtime
        .block_on(async {
            store.verify_integrity().await?;
            let Some(persisted) = store
                .load_existing_commit(
                    &key.commit_id,
                    &key.campaign_id,
                    &key.stream_id,
                    &key.idempotency_key,
                )
                .await?
            else {
                let expected_version = i64::try_from(key.expected_version).map_err(|_| {
                    CanonicalStoreError::IntegrityViolation("expected_version_invalid")
                })?;
                let actual_version: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(max(stream_version), 0) \
                     FROM event_store WHERE campaign_id = $1 AND stream_id = $2",
                )
                .bind(&key.campaign_id)
                .bind(&key.stream_id)
                .fetch_one(&store.primary)
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "preflight_stream_version",
                })?;
                if actual_version != expected_version {
                    return Err(CanonicalStoreError::VersionConflict {
                        expected: expected_version,
                        actual: actual_version,
                    });
                }
                return Ok(None);
            };
            if persisted.commit_id != key.commit_id {
                return Err(CanonicalStoreError::IdempotencyConflict);
            }
            let stored_scope: (String, String, String) = sqlx::query_as(
                "SELECT campaign_id, stream_id, idempotency_key \
                 FROM formal_commits WHERE commit_id = $1",
            )
            .bind(&persisted.commit_id)
            .fetch_one(&store.primary)
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "load_receipt_scope",
            })?;
            if stored_scope
                != (
                    key.campaign_id.clone(),
                    key.stream_id.clone(),
                    key.idempotency_key.clone(),
                )
            {
                return Err(CanonicalStoreError::IdempotencyConflict);
            }
            let events =
                load_committed_events(&store.primary, &store.payload_cipher, &persisted).await?;
            Ok(Some(CanonicalCommitReceipt {
                first_stream_version: u64::try_from(persisted.first_stream_version).map_err(
                    |_| CanonicalStoreError::IntegrityViolation("receipt_version_invalid"),
                )?,
                last_stream_version: u64::try_from(persisted.last_stream_version).map_err(
                    |_| CanonicalStoreError::IntegrityViolation("receipt_version_invalid"),
                )?,
                events,
            }))
        })
        .map_err(map_canonical_port_error)
}

fn commit_on_runtime(
    runtime: &tokio::runtime::Runtime,
    store: &PostgresCanonicalStore,
    draft: &AtomicCommitDraft,
) -> KernelResult<CanonicalCommitReceipt> {
    let (persisted, events) = runtime
        .block_on(async {
            let persisted = store.commit(draft).await?;
            store.verify_integrity().await?;
            let events =
                load_committed_events(&store.primary, &store.payload_cipher, &persisted).await?;
            Ok::<_, CanonicalStoreError>((persisted, events))
        })
        .map_err(map_canonical_port_error)?;
    Ok(CanonicalCommitReceipt {
        first_stream_version: u64::try_from(persisted.first_stream_version)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?,
        last_stream_version: u64::try_from(persisted.last_stream_version)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?,
        events,
    })
}

async fn verify_receipt_on_store(
    store: &PostgresCanonicalStore,
    draft: &AtomicCommitDraft,
    receipt: &CanonicalCommitReceipt,
) -> Result<(), CanonicalStoreError> {
    let normalized = normalize_and_validate(draft)?;
    store.verify_integrity().await?;
    let persisted = store
        .load_existing_commit(
            &normalized.commit_id,
            &normalized.campaign_id,
            &normalized.stream_id,
            &normalized.idempotency_key,
        )
        .await?
        .ok_or(CanonicalStoreError::IntegrityViolation(
            "canonical_receipt_commit_missing",
        ))?;
    let persisted_request_hash: String =
        sqlx::query_scalar("SELECT request_hash FROM formal_commits WHERE commit_id = $1")
            .bind(&normalized.commit_id)
            .fetch_one(&store.primary)
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "verify_receipt_request_hash",
            })?;
    if !stored_request_hash_matches(&normalized, &persisted_request_hash) {
        return Err(CanonicalStoreError::IntegrityViolation(
            "canonical_receipt_request_mismatch",
        ));
    }
    let events = load_committed_events(&store.primary, &store.payload_cipher, &persisted).await?;
    let expected = CanonicalCommitReceipt {
        first_stream_version: u64::try_from(persisted.first_stream_version)
            .map_err(|_| CanonicalStoreError::IntegrityViolation("receipt_version_invalid"))?,
        last_stream_version: u64::try_from(persisted.last_stream_version)
            .map_err(|_| CanonicalStoreError::IntegrityViolation("receipt_version_invalid"))?,
        events,
    };
    if &expected != receipt {
        return Err(CanonicalStoreError::IntegrityViolation(
            "canonical_receipt_bytes_mismatch",
        ));
    }
    Ok(())
}

fn canonical_request_draft(request: &CanonicalCommitRequest) -> KernelResult<AtomicCommitDraft> {
    let expected_version =
        i64::try_from(request.expected_version).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let authority_contract_version = i64::try_from(request.authority_contract_version)
        .map_err(|_| TrpgError::AuthorityContractVersionConflict)?;
    Ok(AtomicCommitDraft {
        commit_id: request.commit_id.clone(),
        campaign_id: request.campaign_id.clone(),
        // The policy audit is constructed from AuthenticatedCommandContext's
        // ResourceRef. Reusing that resource id avoids a second, drift-prone
        // stream field in the external write request while preserving a
        // lossless authorized-resource -> database-stream mapping.
        stream_id: request.audit.resource_id.clone(),
        idempotency_key: request.idempotency_key.clone(),
        expected_version,
        command_id: request.command_id.clone(),
        authenticated_actor_id: request.authenticated_actor_id.clone(),
        authenticated_actor_role: request.authenticated_actor_role.clone(),
        authenticated_actor_origin: request.authenticated_actor_origin.clone(),
        authority_mode: request.authority_mode.clone(),
        authority_contract_version,
        authority_contract_id: request.authority_contract_id.clone(),
        authority_owner: request.authority_owner.clone(),
        visibility_label: request.visibility_label.clone(),
        visibility_subject: request.visibility_subject.clone(),
        data_subject_id: request.data_subject_id.clone(),
        provenance_kind: request.provenance_kind.clone(),
        provenance_reference: request.provenance_reference.clone(),
        provenance_recorded_by: request.provenance_recorded_by.clone(),
        correlation_id: request.correlation_id.clone(),
        causation_id: request.causation_id.clone(),
        trace_id: request.trace_id.clone(),
        events: request
            .events
            .iter()
            .map(|event| CanonicalEventDraft {
                event_type: event.event_type.clone(),
                payload_json: event.payload_json.clone(),
                visibility: None,
                projection_targets: Vec::new(),
            })
            .collect(),
        audit: PolicyAuditDraft {
            actor_id: request.audit.actor_id.clone(),
            actor_origin: request.audit.actor_origin.clone(),
            authentication_reference: request.audit.authentication_reference.clone(),
            resource_type: request.audit.resource_type.clone(),
            resource_id: request.audit.resource_id.clone(),
            action: request.audit.action.clone(),
            requested_role: request.audit.requested_role.clone(),
            openfga_decision_id: request.audit.openfga_decision_id.clone(),
            openfga_policy_revision: request.audit.openfga_policy_revision.clone(),
            opa_decision_id: request.audit.opa_decision_id.clone(),
            opa_policy_revision: request.audit.opa_policy_revision.clone(),
        },
    })
}

fn map_canonical_port_error(error: CanonicalStoreError) -> TrpgError {
    match error {
        CanonicalStoreError::VersionConflict { expected, actual } => {
            match (u64::try_from(expected), u64::try_from(actual)) {
                (Ok(expected), Ok(actual)) => {
                    TrpgError::ExpectedVersionConflict { expected, actual }
                }
                _ => TrpgError::AuditIntegrityViolation,
            }
        }
        CanonicalStoreError::IdempotencyConflict => TrpgError::DuplicateCommand,
        _ => TrpgError::AuditIntegrityViolation,
    }
}
