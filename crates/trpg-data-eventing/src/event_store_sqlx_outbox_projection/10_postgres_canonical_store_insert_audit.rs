
impl PostgresCanonicalStore {

    async fn insert_audit(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        draft: &AtomicCommitDraft,
        event_batch_hash: &str,
        prepared: &WitnessRecord,
    ) -> Result<i64, CanonicalStoreError> {
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('trpg.canonical_audit_log.chain', 0))",
        )
        .execute(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "lock_audit_chain",
        })?;
        let previous = sqlx::query(
            "SELECT sequence, record_hash FROM canonical_audit_log ORDER BY sequence DESC LIMIT 1",
        )
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "read_audit_head",
        })?;
        let (sequence, previous_hash) = previous.map_or((1, GENESIS_HASH.to_owned()), |row| {
            (row.get::<i64, _>("sequence") + 1, row.get("record_hash"))
        });
        let record = AuditRecord {
            sequence,
            commit_id: draft.commit_id.clone(),
            campaign_id: draft.campaign_id.clone(),
            actor_id: draft.audit.actor_id.clone(),
            actor_origin: draft.audit.actor_origin.clone(),
            authentication_reference: draft.audit.authentication_reference.clone(),
            resource_type: draft.audit.resource_type.clone(),
            resource_id: draft.audit.resource_id.clone(),
            action: draft.audit.action.clone(),
            requested_role: draft.audit.requested_role.clone(),
            visibility_label: draft.visibility_label.clone(),
            visibility_subject: draft.visibility_subject.clone(),
            provenance_kind: draft.provenance_kind.clone(),
            provenance_reference: draft.provenance_reference.clone(),
            provenance_recorded_by: draft.provenance_recorded_by.clone(),
            decision: "PERMIT".to_owned(),
            openfga_decision_id: draft.audit.openfga_decision_id.clone(),
            openfga_policy_revision: draft.audit.openfga_policy_revision.clone(),
            opa_decision_id: draft.audit.opa_decision_id.clone(),
            opa_policy_revision: draft.audit.opa_policy_revision.clone(),
            trace_id: draft.trace_id.clone(),
            correlation_id: draft.correlation_id.clone(),
            causation_id: draft.causation_id.clone(),
            event_batch_hash: event_batch_hash.to_owned(),
            witness_prepare_sequence: prepared.sequence,
            witness_prepare_hash: prepared.record_hash.clone(),
            occurred_at: Utc::now(),
            integrity_version: 3,
            key_id: self.integrity_key_id.clone(),
            previous_hash,
            record_hash: String::new(),
        };
        let record_hash = audit_record_hash(self.integrity_key(), &record);

        let inserted_sequence: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO canonical_audit_log (
                sequence, commit_id, campaign_id, actor_id, actor_origin,
                authentication_reference, resource_type, resource_id, action,
                requested_role, visibility_label, visibility_subject, provenance_kind,
                provenance_reference, provenance_recorded_by, decision,
                openfga_decision_id, openfga_policy_revision, opa_decision_id,
                opa_policy_revision, trace_id, correlation_id, causation_id,
                event_batch_hash,
                witness_prepare_sequence, witness_prepare_hash, occurred_at,
                integrity_version, integrity_key_id, previous_hash, record_hash
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24,
                $25, $26, $27, $28, $29, $30, $31
            ) RETURNING sequence
            "#,
        )
        .bind(record.sequence)
        .bind(&record.commit_id)
        .bind(&record.campaign_id)
        .bind(&record.actor_id)
        .bind(&record.actor_origin)
        .bind(&record.authentication_reference)
        .bind(&record.resource_type)
        .bind(&record.resource_id)
        .bind(&record.action)
        .bind(&record.requested_role)
        .bind(&record.visibility_label)
        .bind(&record.visibility_subject)
        .bind(&record.provenance_kind)
        .bind(&record.provenance_reference)
        .bind(&record.provenance_recorded_by)
        .bind(&record.decision)
        .bind(&record.openfga_decision_id)
        .bind(&record.openfga_policy_revision)
        .bind(&record.opa_decision_id)
        .bind(&record.opa_policy_revision)
        .bind(&record.trace_id)
        .bind(&record.correlation_id)
        .bind(&record.causation_id)
        .bind(&record.event_batch_hash)
        .bind(record.witness_prepare_sequence)
        .bind(&record.witness_prepare_hash)
        .bind(record.occurred_at)
        .bind(record.integrity_version)
        .bind(&record.key_id)
        .bind(&record.previous_hash)
        .bind(&record_hash)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "insert_audit",
        })?;
        if inserted_sequence != sequence {
            return Err(CanonicalStoreError::IntegrityViolation(
                "audit_sequence_changed_during_insert",
            ));
        }
        Ok(sequence)
    }

    async fn append_witness(
        &self,
        commit_id: &str,
        phase: WitnessPhase,
        request_hash: &str,
        first_sequence: Option<i64>,
        last_sequence: Option<i64>,
        reason: &str,
    ) -> Result<WitnessRecord, CanonicalStoreError> {
        let mut transaction = self
            .witness
            .begin()
            .await
            .map_err(|_| CanonicalStoreError::WitnessWrite { operation: "begin" })?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended('trpg.external_audit_witness.chain', 0))",
        )
        .execute(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite { operation: "lock" })?;

        // Re-verify under the same transaction-scoped lock that serializes the
        // append. The earlier commit/recovery preflight protects the primary
        // audit chain; this locked verification closes the witness TOCTOU
        // window between that preflight and this mutation.
        let chain_rows = sqlx::query(
            r#"
            SELECT sequence, commit_id, phase, primary_request_hash,
                   primary_first_sequence, primary_last_sequence, reason,
                   integrity_key_id, previous_hash, record_hash
              FROM external_audit_witness ORDER BY sequence
            "#,
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "load_locked_integrity_chain",
        })?;
        let chain = chain_rows.iter().map(witness_from_row).collect::<Vec<_>>();
        verify_witness_chain(&chain, self.integrity_key())?;

        if let Some(row) = sqlx::query(
            r#"
            SELECT sequence, commit_id, phase, primary_request_hash,
                   primary_first_sequence, primary_last_sequence, reason,
                   integrity_key_id, previous_hash, record_hash
              FROM external_audit_witness
             WHERE commit_id = $1 AND phase = $2
            "#,
        )
        .bind(commit_id)
        .bind(phase.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "read_idempotent_record",
        })? {
            let existing = witness_from_row(&row);
            if existing.request_hash != request_hash
                || existing.first_sequence != first_sequence
                || existing.last_sequence != last_sequence
            {
                return Err(CanonicalStoreError::IdempotencyConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| CanonicalStoreError::WitnessWrite {
                    operation: "commit_idempotent_record",
                })?;
            return Ok(existing);
        }

        let previous = sqlx::query(
            "SELECT sequence, record_hash FROM external_audit_witness ORDER BY sequence DESC LIMIT 1",
        )
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "read_head",
        })?;
        let (sequence, previous_hash) = previous.map_or((1, GENESIS_HASH.to_owned()), |row| {
            (row.get::<i64, _>("sequence") + 1, row.get("record_hash"))
        });
        let mut record = WitnessRecord {
            sequence,
            commit_id: commit_id.to_owned(),
            phase: phase.as_str().to_owned(),
            request_hash: request_hash.to_owned(),
            first_sequence,
            last_sequence,
            reason: reason.to_owned(),
            key_id: self.integrity_key_id.clone(),
            previous_hash,
            record_hash: String::new(),
        };
        record.record_hash = witness_record_hash(self.integrity_key(), &record);

        let inserted_sequence: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO external_audit_witness (
                sequence, commit_id, phase, primary_request_hash,
                primary_first_sequence, primary_last_sequence, reason,
                integrity_key_id, previous_hash, record_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING sequence
            "#,
        )
        .bind(record.sequence)
        .bind(&record.commit_id)
        .bind(&record.phase)
        .bind(&record.request_hash)
        .bind(record.first_sequence)
        .bind(record.last_sequence)
        .bind(&record.reason)
        .bind(&record.key_id)
        .bind(&record.previous_hash)
        .bind(&record.record_hash)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "insert_record",
        })?;
        if inserted_sequence != sequence {
            return Err(CanonicalStoreError::IntegrityViolation(
                "witness_sequence_changed_during_insert",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(|_| CanonicalStoreError::WitnessWrite {
                operation: "commit_record",
            })?;
        Ok(record)
    }

    async fn finalize_witness(
        &self,
        persisted: &PersistedCommit,
        request_hash: &str,
    ) -> Result<(), CanonicalStoreError> {
        match self
            .append_witness(
                &persisted.commit_id,
                WitnessPhase::Committed,
                request_hash,
                Some(persisted.first_event_sequence),
                Some(persisted.last_event_sequence),
                "primary_commit_verified",
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(CanonicalStoreError::IntegrityViolation(reason)) => {
                Err(CanonicalStoreError::IntegrityViolation(reason))
            }
            Err(CanonicalStoreError::IdempotencyConflict) => {
                Err(CanonicalStoreError::IdempotencyConflict)
            }
            Err(_) => Err(CanonicalStoreError::WitnessFinalizationPending {
                commit_id: persisted.commit_id.clone(),
            }),
        }
    }

    async fn load_existing_commit(
        &self,
        commit_id: &str,
        campaign_id: &str,
        stream_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<PersistedCommit>, CanonicalStoreError> {
        let row = if idempotency_key.is_empty() {
            sqlx::query(
                r#"
                SELECT commit_id,
                       (response_payload->>'first_event_sequence')::bigint AS first_event_sequence,
                       result_event_sequence AS last_event_sequence,
                       (response_payload->>'first_stream_version')::bigint AS first_stream_version,
                       (response_payload->>'last_stream_version')::bigint AS last_stream_version,
                       audit_sequence,
                       witness_prepare_sequence, witness_prepare_hash
                  FROM formal_commits WHERE commit_id = $1
                "#,
            )
            .bind(commit_id)
            .fetch_optional(&self.primary)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT commit_id,
                       (response_payload->>'first_event_sequence')::bigint AS first_event_sequence,
                       result_event_sequence AS last_event_sequence,
                       (response_payload->>'first_stream_version')::bigint AS first_stream_version,
                       (response_payload->>'last_stream_version')::bigint AS last_stream_version,
                       audit_sequence,
                       witness_prepare_sequence, witness_prepare_hash
                  FROM formal_commits
                 WHERE commit_id = $1
                    OR (
                        campaign_id = $2
                        AND stream_id = $3
                        AND idempotency_operation = 'canonical_commit'
                        AND idempotency_key = $4
                    )
                 ORDER BY CASE WHEN commit_id = $1 THEN 0 ELSE 1 END LIMIT 1
                "#,
            )
            .bind(commit_id)
            .bind(campaign_id)
            .bind(stream_id)
            .bind(idempotency_key)
            .fetch_optional(&self.primary)
            .await
        }
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "load_existing_commit",
        })?;
        Ok(row.map(|row| persisted_from_row(&row)))
    }

    async fn verify_cryptographic_chains(&self) -> Result<(), CanonicalStoreError> {
        let witness_records = self.load_witness_records().await?;
        verify_witness_chain(&witness_records, self.integrity_key())?;
        let audit_records = self.load_audit_records().await?;
        verify_audit_chain(&audit_records, self.integrity_key())
    }
}
