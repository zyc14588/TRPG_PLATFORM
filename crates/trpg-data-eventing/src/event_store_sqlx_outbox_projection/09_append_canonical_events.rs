impl PostgresCanonicalStore {
    async fn append_canonical_events(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        draft: &AtomicCommitDraft,
        request_hash: &str,
    ) -> Result<(Vec<i64>, Vec<String>), CanonicalStoreError> {
        let mut event_sequences = Vec::with_capacity(draft.events.len());
        let mut event_hashes = Vec::with_capacity(draft.events.len());
        for (index, event) in draft.events.iter().enumerate() {
            let event_visibility_label = event
                .visibility
                .as_ref()
                .map_or(draft.visibility_label.as_str(), |value| {
                    value.label.as_str()
                });
            let event_visibility_subject = event
                .visibility
                .as_ref()
                .map_or(draft.visibility_subject.as_str(), |value| {
                    value.subject.as_str()
                });
            let event_data_subject_id = event
                .visibility
                .as_ref()
                .map_or(draft.data_subject_id.as_str(), |value| {
                    value.data_subject_id.as_str()
                });
            let subject_payload_cipher = if event_data_subject_id == "not_applicable" {
                None
            } else {
                Some(
                    load_or_create_subject_payload_cipher(
                        transaction,
                        self.payload_cipher.as_ref(),
                        event_data_subject_id,
                    )
                    .await?,
                )
            };
            let payload_cipher = subject_payload_cipher
                .as_ref()
                .unwrap_or(self.payload_cipher.as_ref());
            let payload: Value = serde_json::from_str(&event.payload_json)
                .map_err(|_| CanonicalStoreError::Validation("event_payload_must_be_json"))?;
            let derivation = rag_derivation_fields(&event.event_type, &payload)?;
            let DeletionRequestFields {
                job_id: deletion_job_id,
                subject_id: deletion_subject_id,
                requested_by: deletion_requested_by,
                retention_policy: deletion_retention_policy,
            } = deletion_request_fields(&event.event_type, &payload)?;
            if let Some(subject_id) = deletion_subject_id.as_deref() {
                if subject_id != event_data_subject_id
                    || subject_id != draft.audit.resource_id
                    || draft.audit.resource_type != "data_subject"
                    || draft.provenance_kind != "user_statement"
                    || deletion_requested_by.as_deref() != Some(&draft.provenance_recorded_by)
                {
                    return Err(CanonicalStoreError::Validation(
                        "deletion_request_binding_invalid",
                    ));
                }
            }
            let canonical_payload = serde_json::to_string(&payload)
                .map_err(|_| CanonicalStoreError::Validation("event_payload_must_be_json"))?;
            let stream_version = draft.expected_version + index as i64 + 1;
            let encrypted_payload = payload_cipher
                .encrypt_json_field(
                    canonical_payload.as_bytes(),
                    &[
                        &draft.campaign_id,
                        &draft.stream_id,
                        &draft.command_id,
                        &event.event_type,
                    ],
                )
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "encrypt_event_payload",
                })?;
            let protected_payload = encrypted_payload.envelope().clone();
            let protected_integrity_source =
                serde_json::to_string(&protected_payload).map_err(|_| {
                    CanonicalStoreError::PrimaryWrite {
                        operation: "serialize_protected_payload",
                    }
                })?;
            let projection_capability = self.derive_core_projection_capability(&draft.commit_id)?;
            let projection_capability_hash = format!(
                "sha256:{:x}",
                Sha256::digest(projection_capability.as_bytes())
            );
            let projection_targets = event
                .projection_targets
                .iter()
                .map(|target| PersistedCanonicalProjectionTarget {
                    relation: target.relation.clone(),
                    row_id: target.row_id.clone(),
                    capability_hash: projection_capability_hash.clone(),
                })
                .collect::<Vec<_>>();
            let projection_targets_json =
                serde_json::to_string(&projection_targets).map_err(|_| {
                    CanonicalStoreError::PrimaryWrite {
                        operation: "serialize_projection_targets",
                    }
                })?;
            let projection_targets = serde_json::to_value(&projection_targets).map_err(|_| {
                CanonicalStoreError::PrimaryWrite {
                    operation: "serialize_projection_targets",
                }
            })?;
            let event_idempotency_key = format!("{}:{index:04}", draft.idempotency_key);
            let sequence: i64 =
                sqlx::query_scalar("SELECT nextval('event_store_sequence_seq'::regclass)")
                    .fetch_one(&mut **transaction)
                    .await
                    .map_err(|_| CanonicalStoreError::PrimaryWrite {
                        operation: "allocate_event_sequence",
                    })?;
            let recorded_at = Utc::now();
            let authenticated_actor_origin =
                serde_json::to_string(&draft.authenticated_actor_origin).map_err(|_| {
                    CanonicalStoreError::PrimaryWrite {
                        operation: "serialize_authenticated_actor_origin",
                    }
                })?;
            let integrity_record = CanonicalEventIntegrityRecord {
                sequence,
                event_index: index,
                stream_version,
                event_type: event.event_type.clone(),
                command_id: draft.command_id.clone(),
                idempotency_key: event_idempotency_key.clone(),
                expected_version: draft.expected_version,
                authority_mode: draft.authority_mode.clone(),
                authority_contract_version: draft.authority_contract_version,
                visibility_label: event_visibility_label.to_owned(),
                provenance_kind: draft.provenance_kind.clone(),
                provenance_reference: draft.provenance_reference.clone(),
                provenance_recorded_by: draft.provenance_recorded_by.clone(),
                correlation_id: draft.correlation_id.clone(),
                causation_id: draft.causation_id.clone(),
                campaign_id: draft.campaign_id.clone(),
                authenticated_actor_id: draft.authenticated_actor_id.clone(),
                authenticated_actor_role: draft.authenticated_actor_role.clone(),
                authenticated_actor_origin,
                resource_type: draft.audit.resource_type.clone(),
                resource_id: draft.audit.resource_id.clone(),
                authority_contract_id: draft.authority_contract_id.clone(),
                authority_owner: draft.authority_owner.clone(),
                visibility_subject: event_visibility_subject.to_owned(),
                trace_id: draft.trace_id.clone(),
                stream_id: draft.stream_id.clone(),
                event_schema_version: crate::persistence::CURRENT_EVENT_SCHEMA_VERSION,
                idempotency_operation: CANONICAL_IDEMPOTENCY_OPERATION.to_owned(),
                request_hash: request_hash.to_owned(),
                request_hash_source: "formal_commit".to_owned(),
                integrity_status: "verified_hmac".to_owned(),
                payload_integrity_source: protected_integrity_source.clone(),
                payload_ciphertext: Some(encrypted_payload.ciphertext().to_vec()),
                payload_key_reference: Some(encrypted_payload.key_reference().as_str().to_owned()),
                payload_nonce: Some(encrypted_payload.nonce().as_slice().to_vec()),
                data_subject_id: event_data_subject_id.to_owned(),
                projection_targets_json: projection_targets_json.clone(),
                recorded_at_micros: recorded_at.timestamp_micros(),
                derived_source_event_sequence: derivation.source_event_sequence,
                derived_snapshot_id: derivation.snapshot_id.clone(),
                derived_chunk_id: derivation.chunk_id.clone(),
                derived_content_hash: derivation.content_hash.clone(),
                derived_source_type: derivation.source_type.clone(),
                derived_copyright_status: derivation.copyright_status.clone(),
                derived_allowed_use: derivation.allowed_use.clone(),
                derived_embedding_model: derivation.embedding_model.clone(),
                derived_embedding_dimensions: derivation.embedding_dimensions,
                derived_embedding_hash: derivation.embedding_hash.clone(),
                deletion_job_id: deletion_job_id.clone(),
                deletion_subject_id: deletion_subject_id.clone(),
                deletion_requested_by: deletion_requested_by.clone(),
                deletion_retention_policy: deletion_retention_policy.clone(),
            };
            let event_hash = event_integrity_hash_v3(self.integrity_key(), &integrity_record);
            let sequence: i64 = sqlx::query_scalar(
                r#"
                INSERT INTO event_store (
                    sequence,
                    event_type, command_id, idempotency_key, expected_version,
                    authority_mode, authority_contract_version, visibility_label,
                    fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
                    correlation_id, causation_id, payload_json, campaign_id,
                    stream_version, authenticated_actor_id, authenticated_actor_role,
                    authenticated_actor_origin, resource_type, resource_id,
                    authority_contract_id, authority_owner, visibility_subject, trace_id,
                    event_integrity_hash, stream_id, event_schema_version,
                    idempotency_operation, request_hash, request_hash_source,
                    integrity_status, payload_integrity_source,
                    payload_ciphertext, payload_key_reference, payload_nonce,
                    data_subject_id, derived_source_event_sequence,
                    derived_snapshot_id, derived_chunk_id, derived_content_hash,
                    derived_source_type, derived_copyright_status,
                    derived_allowed_use, derived_embedding_model,
                    derived_embedding_dimensions, derived_embedding_hash,
                    deletion_job_id, deletion_subject_id, deletion_requested_by,
                    deletion_retention_policy, projection_targets,
                    event_integrity_version, recorded_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                    $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23,
                    $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34,
                    $35, $36, $37, $38, $39, $40, $41, $42, $43, $44, $45,
                    $46, $47, $48, $49, $50, $51, $52, $53, $54
                ) RETURNING sequence
                "#,
            )
            .bind(sequence)
            .bind(&event.event_type)
            .bind(&draft.command_id)
            .bind(&event_idempotency_key)
            .bind(draft.expected_version)
            .bind(&draft.authority_mode)
            .bind(draft.authority_contract_version)
            .bind(event_visibility_label)
            .bind(&draft.provenance_kind)
            .bind(&draft.provenance_reference)
            .bind(&draft.provenance_recorded_by)
            .bind(&draft.correlation_id)
            .bind(&draft.causation_id)
            .bind(Json(protected_payload.clone()))
            .bind(&draft.campaign_id)
            .bind(stream_version)
            .bind(&draft.authenticated_actor_id)
            .bind(&draft.authenticated_actor_role)
            .bind(Json(draft.authenticated_actor_origin.clone()))
            .bind(&draft.audit.resource_type)
            .bind(&draft.audit.resource_id)
            .bind(&draft.authority_contract_id)
            .bind(&draft.authority_owner)
            .bind(event_visibility_subject)
            .bind(&draft.trace_id)
            .bind(&event_hash)
            .bind(&draft.stream_id)
            .bind(crate::persistence::CURRENT_EVENT_SCHEMA_VERSION)
            .bind(CANONICAL_IDEMPOTENCY_OPERATION)
            .bind(request_hash)
            .bind("formal_commit")
            .bind("verified_hmac")
            .bind(&protected_integrity_source)
            .bind(encrypted_payload.ciphertext())
            .bind(encrypted_payload.key_reference().as_str())
            .bind(encrypted_payload.nonce().as_slice())
            .bind(event_data_subject_id)
            .bind(derivation.source_event_sequence)
            .bind(derivation.snapshot_id)
            .bind(derivation.chunk_id)
            .bind(derivation.content_hash)
            .bind(derivation.source_type)
            .bind(derivation.copyright_status)
            .bind(derivation.allowed_use)
            .bind(derivation.embedding_model)
            .bind(derivation.embedding_dimensions)
            .bind(derivation.embedding_hash)
            .bind(deletion_job_id)
            .bind(deletion_subject_id)
            .bind(deletion_requested_by)
            .bind(deletion_retention_policy)
            .bind(Json(projection_targets))
            .bind(CURRENT_EVENT_INTEGRITY_VERSION)
            .bind(recorded_at)
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "insert_event",
            })?;

            sqlx::query(
                r#"
                INSERT INTO event_outbox (
                    event_id, event_sequence, nats_subject, idempotency_key,
                    visibility_label, correlation_id, causation_id, payload_json, commit_id,
                    campaign_id, stream_id, event_schema_version,
                    idempotency_operation, request_hash, request_hash_source,
                    integrity_status, visibility_subject, payload_ciphertext,
                    payload_key_reference, payload_nonce, data_subject_id
                ) VALUES (
                    $1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                    $12, $13, $14, $15, $16, $17, $18, $19, $20
                )
                "#,
            )
            .bind(sequence)
            .bind(crate::NATS_EVENTS_APPENDED)
            .bind(format!("outbox:{event_idempotency_key}"))
            .bind(event_visibility_label)
            .bind(&draft.correlation_id)
            .bind(&draft.causation_id)
            .bind(Json(protected_payload))
            .bind(&draft.commit_id)
            .bind(&draft.campaign_id)
            .bind(&draft.stream_id)
            .bind(crate::persistence::CURRENT_EVENT_SCHEMA_VERSION)
            .bind(CANONICAL_IDEMPOTENCY_OPERATION)
            .bind(request_hash)
            .bind("formal_commit")
            .bind("verified_hmac")
            .bind(event_visibility_subject)
            .bind(encrypted_payload.ciphertext())
            .bind(encrypted_payload.key_reference().as_str())
            .bind(encrypted_payload.nonce().as_slice())
            .bind(event_data_subject_id)
            .execute(&mut **transaction)
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "insert_outbox",
            })?;

            event_sequences.push(sequence);
            event_hashes.push(event_hash);
        }

        Ok((event_sequences, event_hashes))
    }
}
