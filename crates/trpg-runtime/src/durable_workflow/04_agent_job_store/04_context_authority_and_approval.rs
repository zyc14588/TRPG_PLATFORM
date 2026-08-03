impl DurableWorkflowStore {
    pub async fn load_agent_job_context(
        &self,
        job_id: &str,
    ) -> Result<DurableAgentContextSnapshot, WorkflowStoreError> {
        let input_payload_json: String = sqlx::query_scalar(
            "SELECT source.payload_json::text \
               FROM agent_jobs AS job \
               JOIN event_store AS source \
                 ON source.sequence = job.input_event_sequence \
              WHERE job.job_id = $1 \
                AND source.campaign_id = job.campaign_id \
                AND source.stream_version = job.input_stream_version \
                AND source.integrity_status = 'verified_hmac'",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_job_input"))?
        .ok_or(WorkflowStoreError::IntegrityViolation(
            "agent_job_input_event_invalid",
        ))?;
        let rows = sqlx::query(
            r#"
            SELECT chunk.chunk_id, chunk.source_event_sequence,
                   chunk.visibility AS visibility_label,
                   NULLIF(chunk.visibility_subject, 'not_applicable')
                       AS visibility_subject,
                   chunk.fact_provenance::text AS fact_provenance_json,
                   chunk.chunk_hash, chunk.content
              FROM agent_jobs AS job
              JOIN rag_snapshot_chunk AS chunk
                ON chunk.campaign_id = job.campaign_id
               AND chunk.snapshot_id = job.rag_snapshot_id
             WHERE job.job_id = $1
               AND (job.visibility_scope -> 'allowed_labels') ? chunk.visibility
               AND (
                   chunk.visibility_subject IS NULL
                   OR chunk.visibility_subject = 'not_applicable'
                   OR chunk.visibility_subject =
                      job.visibility_scope ->> 'subject_id'
               )
             ORDER BY chunk.source_event_sequence, chunk.chunk_id
             LIMIT 128
            "#,
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_job_rag"))?;
        let chunks = rows
            .into_iter()
            .map(|row| DurableAgentContextChunk {
                chunk_id: row.get("chunk_id"),
                source_event_sequence: row.get("source_event_sequence"),
                visibility_label: row.get("visibility_label"),
                visibility_subject: row.get("visibility_subject"),
                fact_provenance_json: row.get("fact_provenance_json"),
                chunk_hash: row.get("chunk_hash"),
                content: row.get("content"),
            })
            .collect();
        Ok(DurableAgentContextSnapshot {
            input_payload_json,
            chunks,
        })
    }

    pub async fn load_agent_authority_snapshot(
        &self,
        campaign_id: &str,
    ) -> Result<DurableAgentAuthoritySnapshot, WorkflowStoreError> {
        let row = sqlx::query(
            r#"
            SELECT contract_id, campaign_id, authority_mode, authority_owner,
                   contract_version, prompt_version, agent_pack_version,
                   tool_schema_version, model_route_snapshot
              FROM authority_contracts
             WHERE campaign_id = $1 AND locked
            "#,
        )
        .bind(campaign_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_authority"))?
        .ok_or(WorkflowStoreError::NotFound)?;
        Ok(DurableAgentAuthoritySnapshot {
            contract_id: row.get("contract_id"),
            campaign_id: row.get("campaign_id"),
            authority_mode: row.get("authority_mode"),
            authority_owner: row.get("authority_owner"),
            contract_version: row.get("contract_version"),
            prompt_version: row.get("prompt_version"),
            agent_pack_version: row.get("agent_pack_version"),
            tool_schema_version: row.get("tool_schema_version"),
            model_route_snapshot: row.get("model_route_snapshot"),
        })
    }

    pub async fn load_agent_job_approval(
        &self,
        job_id: &str,
    ) -> Result<Option<DurableAgentApproval>, WorkflowStoreError> {
        let row = sqlx::query(
            r#"
            SELECT approval.approval_id, approval.approval_event_sequence,
                   approval.approved_by, approval.idempotency_key
              FROM agent_job_approvals AS approval
              JOIN agent_jobs AS job ON job.job_id = approval.job_id
              JOIN event_store AS event
                ON event.sequence = approval.approval_event_sequence
               AND event.campaign_id = job.campaign_id
               AND event.event_type = 'AgentDraftApproved'
               AND event.integrity_status = 'verified_hmac'
             WHERE approval.job_id = $1
            "#,
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_job_approval"))?;
        Ok(row.map(|row| DurableAgentApproval {
            approval_id: row.get("approval_id"),
            approval_event_sequence: row.get("approval_event_sequence"),
            approved_by: row.get("approved_by"),
            idempotency_key: row.get("idempotency_key"),
        }))
    }

    pub async fn record_agent_job_approval(
        &self,
        draft: &AgentJobApprovalDraft,
    ) -> Result<DurableAgentApproval, WorkflowStoreError> {
        for (value, reason) in [
            (&draft.approval_id, "approval_id_required"),
            (&draft.job_id, "job_id_required"),
            (&draft.approved_by, "approved_by_required"),
            (&draft.idempotency_key, "approval_idempotency_key_required"),
        ] {
            validate_identifier(value, reason)?;
        }
        if draft.approval_event_sequence <= 0 {
            return Err(WorkflowStoreError::Validation(
                "approval_event_sequence_invalid",
            ));
        }
        let inserted = sqlx::query(
            r#"
            INSERT INTO agent_job_approvals (
                approval_id, job_id, approval_event_sequence,
                approved_by, idempotency_key
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&draft.approval_id)
        .bind(&draft.job_id)
        .bind(draft.approval_event_sequence)
        .bind(&draft.approved_by)
        .bind(&draft.idempotency_key)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("record_agent_job_approval"))?;
        let approval = self
            .load_agent_job_approval(&draft.job_id)
            .await?
            .ok_or(WorkflowStoreError::IntegrityViolation(
                "agent_job_approval_missing",
            ))?;
        if inserted.rows_affected() == 0
            && (approval.approval_id != draft.approval_id
                || approval.approval_event_sequence != draft.approval_event_sequence
                || approval.approved_by != draft.approved_by
                || approval.idempotency_key != draft.idempotency_key)
        {
            return Err(WorkflowStoreError::IdempotencyConflict);
        }
        Ok(approval)
    }

}
