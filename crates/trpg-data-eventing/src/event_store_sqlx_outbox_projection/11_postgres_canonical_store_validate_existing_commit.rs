
impl PostgresCanonicalStore {

    async fn validate_existing_commit(
        &self,
        persisted: &PersistedCommit,
        draft: &AtomicCommitDraft,
        request_hash: &str,
    ) -> Result<String, CanonicalStoreError> {
        if persisted.commit_id != draft.commit_id {
            return Err(CanonicalStoreError::IdempotencyConflict);
        }
        let stored_hash: String =
            sqlx::query_scalar("SELECT request_hash FROM formal_commits WHERE commit_id = $1")
                .bind(&persisted.commit_id)
                .fetch_one(&self.primary)
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "load_existing_request_hash",
                })?;
        if stored_hash != request_hash && !stored_request_hash_matches(draft, &stored_hash) {
            return Err(CanonicalStoreError::IdempotencyConflict);
        }
        let prepared = sqlx::query(
            r#"
            SELECT sequence, record_hash, primary_request_hash
              FROM external_audit_witness
             WHERE commit_id = $1 AND phase = 'PREPARED'
            "#,
        )
        .bind(&persisted.commit_id)
        .fetch_optional(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "load_existing_prepare",
        })?
        .ok_or(CanonicalStoreError::IntegrityViolation(
            "primary_commit_missing_witness_prepare",
        ))?;
        if prepared.get::<i64, _>("sequence") != persisted.witness_prepare_sequence
            || prepared.get::<String, _>("record_hash") != persisted.witness_prepare_hash
            || prepared.get::<String, _>("primary_request_hash") != stored_hash
        {
            return Err(CanonicalStoreError::IntegrityViolation(
                "primary_witness_prepare_binding_mismatch",
            ));
        }
        Ok(stored_hash)
    }

    async fn load_witness_records(&self) -> Result<Vec<WitnessRecord>, CanonicalStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT sequence, commit_id, phase, primary_request_hash,
                   primary_first_sequence, primary_last_sequence, reason,
                   integrity_key_id, previous_hash, record_hash
              FROM external_audit_witness ORDER BY sequence
            "#,
        )
        .fetch_all(&self.witness)
        .await
        .map_err(|_| CanonicalStoreError::WitnessWrite {
            operation: "load_integrity_chain",
        })?;
        Ok(rows.iter().map(witness_from_row).collect())
    }

    async fn load_audit_records(&self) -> Result<Vec<AuditRecord>, CanonicalStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT sequence, commit_id, campaign_id, actor_id, actor_origin,
                   authentication_reference, resource_type, resource_id, action,
                   requested_role, visibility_label, visibility_subject,
                   provenance_kind, provenance_reference, provenance_recorded_by,
                   decision, openfga_decision_id, openfga_policy_revision,
                   opa_decision_id, opa_policy_revision, trace_id,
                   correlation_id, causation_id, event_batch_hash,
                   witness_prepare_sequence, witness_prepare_hash, occurred_at,
                   integrity_version, integrity_key_id, previous_hash, record_hash
              FROM canonical_audit_log ORDER BY sequence
            "#,
        )
        .fetch_all(&self.primary)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "load_audit_integrity_chain",
        })?;
        Ok(rows
            .iter()
            .map(|row| AuditRecord {
                sequence: row.get("sequence"),
                commit_id: row.get("commit_id"),
                campaign_id: row.get("campaign_id"),
                actor_id: row.get("actor_id"),
                actor_origin: row.get("actor_origin"),
                authentication_reference: row.get("authentication_reference"),
                resource_type: row.get("resource_type"),
                resource_id: row.get("resource_id"),
                action: row.get("action"),
                requested_role: row.get("requested_role"),
                visibility_label: row.get("visibility_label"),
                visibility_subject: row.get("visibility_subject"),
                provenance_kind: row.get("provenance_kind"),
                provenance_reference: row.get("provenance_reference"),
                provenance_recorded_by: row.get("provenance_recorded_by"),
                decision: row.get("decision"),
                openfga_decision_id: row.get("openfga_decision_id"),
                openfga_policy_revision: row.get("openfga_policy_revision"),
                opa_decision_id: row.get("opa_decision_id"),
                opa_policy_revision: row.get("opa_policy_revision"),
                trace_id: row.get("trace_id"),
                correlation_id: row.get("correlation_id"),
                causation_id: row.get("causation_id"),
                event_batch_hash: row.get("event_batch_hash"),
                witness_prepare_sequence: row.get("witness_prepare_sequence"),
                witness_prepare_hash: row.get("witness_prepare_hash"),
                occurred_at: row.get("occurred_at"),
                integrity_version: row.get("integrity_version"),
                key_id: row.get("integrity_key_id"),
                previous_hash: row.get("previous_hash"),
                record_hash: row.get("record_hash"),
            })
            .collect())
    }
}

fn nonempty_subject(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty() && value != "not_applicable").then_some(value)
}

fn subject_key_reference(subject_id: &str) -> String {
    let digest = Sha256::digest(subject_id.as_bytes());
    format!("subject_payload_{}", lowercase_hex(&digest))
}

fn subject_key_aad<'a>(subject_id: &'a str, key_reference: &'a str) -> [&'a str; 3] {
    ["subject_payload_key_v1", subject_id, key_reference]
}

async fn load_or_create_subject_payload_cipher(
    transaction: &mut Transaction<'_, Postgres>,
    master_cipher: &PayloadCipher,
    subject_id: &str,
) -> Result<PayloadCipher, CanonicalStoreError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('privacy_subject_key:' || $1, 0))")
        .bind(subject_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "lock_subject_payload_key",
        })?;

    let key_reference = subject_key_reference(subject_id);
    let existing = sqlx::query(
        "SELECT key_reference, wrapped_key, destroyed_at IS NOT NULL AS destroyed \
         FROM privacy_subject_keys WHERE subject_id = $1 FOR UPDATE",
    )
    .bind(subject_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| CanonicalStoreError::PrimaryWrite {
        operation: "load_subject_payload_key",
    })?;

    let mut plaintext_key = Zeroizing::new([0_u8; 32]);
    if let Some(row) = existing {
        let persisted_reference: String = row.get("key_reference");
        let destroyed: bool = row.get("destroyed");
        let wrapped_key: Option<Vec<u8>> = row.get("wrapped_key");
        if destroyed || persisted_reference != key_reference {
            return Err(CanonicalStoreError::Validation("data_subject_deleted"));
        }
        let wrapped_key = wrapped_key.ok_or(CanonicalStoreError::IntegrityViolation(
            "subject_key_material_missing",
        ))?;
        let envelope: Value = serde_json::from_slice(&wrapped_key)
            .map_err(|_| CanonicalStoreError::IntegrityViolation("subject_key_envelope_invalid"))?;
        let unwrapped = master_cipher
            .decrypt_json(&envelope, &subject_key_aad(subject_id, &key_reference))
            .map_err(|_| CanonicalStoreError::IntegrityViolation("subject_key_unwrap_failed"))?;
        if unwrapped.as_bytes().len() != plaintext_key.len() {
            return Err(CanonicalStoreError::IntegrityViolation(
                "subject_key_length_invalid",
            ));
        }
        plaintext_key.copy_from_slice(unwrapped.as_bytes());
    } else {
        SystemRandom::new()
            .fill(plaintext_key.as_mut_slice())
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "generate_subject_payload_key",
            })?;
        let wrapped = master_cipher
            .encrypt_json_field(
                plaintext_key.as_slice(),
                &subject_key_aad(subject_id, &key_reference),
            )
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "wrap_subject_payload_key",
            })?;
        let encoded = serde_json::to_vec(wrapped.envelope()).map_err(|_| {
            CanonicalStoreError::PrimaryWrite {
                operation: "serialize_subject_payload_key",
            }
        })?;
        sqlx::query(
            "INSERT INTO privacy_subject_keys \
             (subject_id, key_reference, wrapped_key) VALUES ($1, $2, $3)",
        )
        .bind(subject_id)
        .bind(&key_reference)
        .bind(encoded)
        .execute(&mut **transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "persist_subject_payload_key",
        })?;
    }

    PayloadCipher::new(key_reference, plaintext_key.as_slice())
        .map_err(|_| CanonicalStoreError::IntegrityViolation("subject_payload_cipher_invalid"))
}

async fn load_subject_payload_cipher(
    pool: &PgPool,
    master_cipher: &PayloadCipher,
    subject_id: &str,
    event_key_reference: &str,
) -> Result<Option<PayloadCipher>, CanonicalStoreError> {
    if subject_id == "not_applicable" {
        if event_key_reference != master_cipher.key_reference().as_str() {
            return Err(CanonicalStoreError::IntegrityViolation(
                "unscoped_payload_key_reference_mismatch",
            ));
        }
        return Ok(None);
    }
    let expected_reference = subject_key_reference(subject_id);
    if event_key_reference != expected_reference {
        return Err(CanonicalStoreError::IntegrityViolation(
            "subject_payload_key_reference_mismatch",
        ));
    }
    let row = sqlx::query(
        "SELECT key_reference, wrapped_key, destroyed_at IS NOT NULL AS destroyed \
         FROM privacy_subject_keys WHERE subject_id = $1",
    )
    .bind(subject_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| CanonicalStoreError::PrimaryWrite {
        operation: "load_subject_payload_key",
    })?
    .ok_or(CanonicalStoreError::IntegrityViolation(
        "subject_payload_key_missing",
    ))?;
    let key_reference: String = row.get("key_reference");
    let destroyed: bool = row.get("destroyed");
    let wrapped_key: Option<Vec<u8>> = row.get("wrapped_key");
    if destroyed {
        return Err(CanonicalStoreError::Validation("data_subject_deleted"));
    }
    if key_reference != expected_reference {
        return Err(CanonicalStoreError::IntegrityViolation(
            "subject_payload_key_identity_mismatch",
        ));
    }
    let envelope: Value = serde_json::from_slice(&wrapped_key.ok_or(
        CanonicalStoreError::IntegrityViolation("subject_key_material_missing"),
    )?)
    .map_err(|_| CanonicalStoreError::IntegrityViolation("subject_key_envelope_invalid"))?;
    let unwrapped = master_cipher
        .decrypt_json(&envelope, &subject_key_aad(subject_id, &key_reference))
        .map_err(|_| CanonicalStoreError::IntegrityViolation("subject_key_unwrap_failed"))?;
    if unwrapped.as_bytes().len() != 32 {
        return Err(CanonicalStoreError::IntegrityViolation(
            "subject_key_length_invalid",
        ));
    }
    PayloadCipher::new(key_reference, unwrapped.as_bytes())
        .map(Some)
        .map_err(|_| CanonicalStoreError::IntegrityViolation("subject_payload_cipher_invalid"))
}

fn persisted_provenance_kind(value: &str) -> Result<ProvenanceKind, CanonicalStoreError> {
    match value {
        "human_keeper_statement" => Ok(ProvenanceKind::HumanKeeperStatement),
        "user_statement" => Ok(ProvenanceKind::UserStatement),
        "rules_engine_decision" => Ok(ProvenanceKind::RulesEngineDecision),
        "tool_result" => Ok(ProvenanceKind::ToolResult),
        "agent_proposal" => Ok(ProvenanceKind::AgentProposal),
        "imported_source" => Ok(ProvenanceKind::ImportedSource),
        "system_fixture" => Ok(ProvenanceKind::SystemFixture),
        _ => Err(CanonicalStoreError::Validation(
            "committed_fact_provenance_invalid",
        )),
    }
}

fn valid_hmac_hash(value: &str) -> bool {
    const PREFIX: &str = "hmac-sha256:";
    value.len() == PREFIX.len() + 64
        && value.starts_with(PREFIX)
        && value[PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
