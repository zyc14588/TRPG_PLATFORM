
impl PostgresCloudEgressLedger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn load_recorded_decision(
        &self,
        snapshot_id: &EntityId,
    ) -> KernelResult<(String, Option<String>, String)> {
        let row = sqlx::query(
            "SELECT route.decision, route.denial_code, audit.context_manifest_hash \
             FROM cloud_egress_route_snapshots route \
             JOIN cloud_egress_audit audit ON audit.snapshot_id = route.snapshot_id \
             WHERE route.snapshot_id = $1",
        )
        .bind(snapshot_id.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        Ok((
            row.try_get("decision")
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
            row.try_get("denial_code")
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
            row.try_get("context_manifest_hash")
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
        ))
    }
}

#[async_trait]
impl CloudEgressLedger for PostgresCloudEgressLedger {
    async fn trusted_now_unix_ms(&self) -> KernelResult<u64> {
        let now: i64 = sqlx::query_scalar(
            "SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)?;
        u64::try_from(now).map_err(|_| TrpgError::PolicyEvidenceUntrusted)
    }

    async fn notice_is_recorded(
        &self,
        notice_reference: &EntityId,
        subject_id: &EntityId,
        policy_version: &EntityId,
    ) -> KernelResult<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM cloud_egress_notices \
             WHERE notice_reference = $1 AND subject_id = $2 AND policy_version = $3)",
        )
        .bind(notice_reference.as_str())
        .bind(subject_id.as_str())
        .bind(policy_version.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)
    }

    async fn load_active_consent(
        &self,
        query: &CloudConsentQuery,
    ) -> KernelResult<Option<PersistedCloudConsent>> {
        let now =
            i64::try_from(query.now_unix_ms).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        let row = sqlx::query(
            "SELECT consent_id, subject_id, target_provider, purpose, policy_version, \
             visibility_scope, expires_at_unix_ms \
             FROM cloud_egress_consents \
             WHERE subject_id = $1 AND target_provider = $2 AND purpose = $3 \
               AND policy_version = $4 AND granted = true AND expires_at_unix_ms > $5 \
             ORDER BY updated_at DESC, consent_id DESC LIMIT 1",
        )
        .bind(query.subject_id.as_str())
        .bind(query.target_provider.as_str())
        .bind(query.purpose.as_str())
        .bind(query.policy_version.as_str())
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let expires_at: i64 = row
            .try_get("expires_at_unix_ms")
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        let expires_at =
            u64::try_from(expires_at).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        let id = |column: &str| -> KernelResult<EntityId> {
            let value: String = row
                .try_get(column)
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
            EntityId::new(value).map_err(|_| TrpgError::PolicyEvidenceUntrusted)
        };
        let visibility_scope: String = row
            .try_get("visibility_scope")
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        Ok(Some(PersistedCloudConsent::loaded_from_repository(
            id("consent_id")?,
            id("subject_id")?,
            id("target_provider")?,
            id("purpose")?,
            id("policy_version")?,
            ConsentVisibilityScope::parse(&visibility_scope)?,
            expires_at,
        )))
    }

    async fn record_route_decision(
        &self,
        mut snapshot: CloudRouteSnapshotRecord,
        mut audit: CloudEgressAuditRecord,
    ) -> KernelResult<bool> {
        if snapshot.snapshot_id != audit.snapshot_id
            || snapshot.subject_id != audit.subject_id
            || snapshot.source_provider != audit.source_provider
            || snapshot.target_provider != audit.target_provider
            || snapshot.source_endpoint != audit.source_endpoint
            || snapshot.target_endpoint != audit.target_endpoint
            || snapshot.model_id != audit.model_id
            || snapshot.source_credential_id != audit.source_credential_id
            || snapshot.source_credential_version != audit.source_credential_version
            || snapshot.target_credential_id != audit.target_credential_id
            || snapshot.target_credential_version != audit.target_credential_version
            || snapshot.fallback_policy != audit.fallback_policy
            || snapshot.privacy_boundary != audit.privacy_boundary
            || snapshot.decision != audit.decision
            || snapshot.denial_code != audit.denial_code
            || snapshot.context_manifest_hash != audit.context_manifest_hash
            || snapshot.created_at_unix_ms != audit.created_at_unix_ms
        {
            return Err(TrpgError::PolicyEvidenceUntrusted);
        }
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| TrpgError::PolicyUnavailable)?;
        let mut persisted_as_candidate = true;
        if snapshot.decision == CloudEgressDecision::Allow {
            let Some(consent_id) = snapshot.consent_id.as_ref() else {
                return Err(TrpgError::PolicyEvidenceUntrusted);
            };
            let consent_expires_at = snapshot
                .consent_expires_at_unix_ms
                .ok_or(TrpgError::PolicyEvidenceUntrusted)
                .and_then(|value| {
                    i64::try_from(value).map_err(|_| TrpgError::PolicyEvidenceUntrusted)
                })?;
            let notice_reference = snapshot
                .notice_reference
                .as_ref()
                .ok_or(TrpgError::PolicyEvidenceUntrusted)?;
            let locked_consent = sqlx::query_scalar::<_, i64>(
                "SELECT expires_at_unix_ms FROM cloud_egress_consents \
                 WHERE consent_id = $1 AND subject_id = $2 AND target_provider = $3 \
                   AND purpose = $4 AND policy_version = $5 AND granted = true \
                   AND expires_at_unix_ms = $6 \
                   AND expires_at_unix_ms > \
                       floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint \
                 FOR SHARE",
            )
            .bind(consent_id.as_str())
            .bind(snapshot.subject_id.as_str())
            .bind(snapshot.target_provider.as_str())
            .bind(snapshot.purpose.as_str())
            .bind(snapshot.policy_version.as_str())
            .bind(consent_expires_at)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| TrpgError::PolicyUnavailable)?;
            let locked_notice = sqlx::query_scalar::<_, String>(
                "SELECT notice_reference FROM cloud_egress_notices \
                 WHERE notice_reference = $1 AND subject_id = $2 AND policy_version = $3 \
                 FOR SHARE",
            )
            .bind(notice_reference.as_str())
            .bind(snapshot.subject_id.as_str())
            .bind(snapshot.policy_version.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| TrpgError::PolicyUnavailable)?;
            let still_active = locked_consent == Some(consent_expires_at)
                && locked_notice.as_deref() == Some(notice_reference.as_str());
            if !still_active {
                persisted_as_candidate = false;
                snapshot.decision = CloudEgressDecision::Deny;
                snapshot.denial_code = Some(CloudEgressDenial::ConsentChanged.code());
                snapshot.allowed_fact_ids.clear();
                audit.decision = CloudEgressDecision::Deny;
                audit.denial_code = Some(CloudEgressDenial::ConsentChanged.code());
            }
        }
        let allowed_fact_ids = serde_json::Value::Array(
            snapshot
                .allowed_fact_ids
                .iter()
                .map(|id| serde_json::Value::String(id.to_string()))
                .collect(),
        );
        let created_at = i64::try_from(snapshot.created_at_unix_ms)
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
        sqlx::query(
            "INSERT INTO cloud_egress_route_snapshots (\
             snapshot_id, subject_id, consent_id, source_provider, target_provider, purpose, \
             policy_version, notice_reference, context_manifest_hash, allowed_fact_ids, \
             decision, denial_code, created_at_unix_ms, source_endpoint, target_endpoint, model_id, \
             source_credential_id, source_credential_version, target_credential_id, \
             target_credential_version, fallback_policy, privacy_boundary, \
             consent_expires_at_unix_ms) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
                     $17, $18, $19, $20, $21, $22, $23)",
        )
        .bind(snapshot.snapshot_id.as_str())
        .bind(snapshot.subject_id.as_str())
        .bind(snapshot.consent_id.as_ref().map(EntityId::as_str))
        .bind(snapshot.source_provider.as_str())
        .bind(snapshot.target_provider.as_str())
        .bind(snapshot.purpose.as_str())
        .bind(snapshot.policy_version.as_str())
        .bind(snapshot.notice_reference.as_ref().map(EntityId::as_str))
        .bind(&snapshot.context_manifest_hash)
        .bind(allowed_fact_ids)
        .bind(snapshot.decision.as_str())
        .bind(snapshot.denial_code)
        .bind(created_at)
        .bind(&snapshot.source_endpoint)
        .bind(&snapshot.target_endpoint)
        .bind(snapshot.model_id.as_str())
        .bind(&snapshot.source_credential_id)
        .bind(i64::try_from(snapshot.source_credential_version)
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?)
        .bind(&snapshot.target_credential_id)
        .bind(i64::try_from(snapshot.target_credential_version)
            .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?)
        .bind(snapshot.fallback_policy.as_str())
        .bind(snapshot.privacy_boundary.as_str())
        .bind(
            snapshot
                .consent_expires_at_unix_ms
                .map(i64::try_from)
                .transpose()
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
        )
        .execute(&mut *transaction)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)?;
        sqlx::query(
            "INSERT INTO cloud_egress_audit (\
             audit_id, snapshot_id, subject_id, decision, denial_code, \
             context_manifest_hash, created_at_unix_ms, source_provider, target_provider, \
             source_endpoint, target_endpoint, model_id, source_credential_id, \
             source_credential_version, target_credential_id, target_credential_version, \
             fallback_policy, privacy_boundary) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, \
                     $13, $14, $15, $16, $17, $18)",
        )
        .bind(audit.audit_id.as_str())
        .bind(audit.snapshot_id.as_str())
        .bind(audit.subject_id.as_str())
        .bind(audit.decision.as_str())
        .bind(audit.denial_code)
        .bind(&audit.context_manifest_hash)
        .bind(created_at)
        .bind(audit.source_provider.as_str())
        .bind(audit.target_provider.as_str())
        .bind(&audit.source_endpoint)
        .bind(&audit.target_endpoint)
        .bind(audit.model_id.as_str())
        .bind(&audit.source_credential_id)
        .bind(
            i64::try_from(audit.source_credential_version)
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
        )
        .bind(&audit.target_credential_id)
        .bind(
            i64::try_from(audit.target_credential_version)
                .map_err(|_| TrpgError::PolicyEvidenceUntrusted)?,
        )
        .bind(audit.fallback_policy.as_str())
        .bind(audit.privacy_boundary.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(|_| TrpgError::PolicyUnavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| TrpgError::PolicyUnavailable)?;
        Ok(persisted_as_candidate)
    }
}

#[derive(Clone)]
pub struct PostgresDeletionRepository {
    pool: PgPool,
}

/// Canonical event and workflow fields that must be persisted atomically when
/// a deletion request becomes executable.
pub struct ConfirmedDeletionRecord<'a> {
    pub job_id: &'a str,
    pub subject_id: &'a str,
    pub requested_by: &'a str,
    pub retention_policy: &'a str,
    pub evidence: &'a DeletionRequestEvidence,
    pub canonical_event_sequence: u64,
    pub canonical_event_integrity_hash: &'a str,
}

#[async_trait]
pub trait DeletionRequestPort: Send + Sync {
    async fn request_deletion(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
    ) -> Result<DeletionJob, PrivacyError>;

    async fn confirm_deletion_evidence(
        &self,
        job_id: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError>;

    /// Atomically creates the workflow job and all required targets from an
    /// already verified canonical event. Production callers use this method so
    /// a failed canonical commit cannot leave a pending side-table job.
    async fn record_confirmed_deletion(
        &self,
        record: ConfirmedDeletionRecord<'_>,
    ) -> Result<DeletionJob, PrivacyError>;
}
