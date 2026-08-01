impl DurableWorkflowStore {
    pub async fn transition_agent_job(
        &self,
        draft: &AgentJobTransitionDraft,
    ) -> Result<DurableAgentJob, WorkflowStoreError> {
        validate_agent_job_transition(draft)?;
        let decision_json = draft
            .decision_json
            .as_deref()
            .map(normalize_json)
            .transpose()?;
        let tool_result_json = draft
            .tool_result_json
            .as_deref()
            .map(normalize_json)
            .transpose()?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkflowStoreError::Database("begin_agent_job_transition"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&draft.job_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkflowStoreError::Database("lock_agent_job_transition"))?;

        if let Some(existing) =
            load_transition_by_idempotency(&mut transaction, &draft.idempotency_key).await?
        {
            if existing.workflow_id != draft.job_id
                || existing.from_state != draft.from_state
                || existing.to_state != draft.to_state
                || existing.workflow_version != draft.expected_version + 1
                || existing.correlation_id != draft.correlation_id
                || existing.causation_id != draft.causation_id
            {
                return Err(WorkflowStoreError::IdempotencyConflict);
            }
            let job = load_agent_job_in_transaction(&mut transaction, &draft.job_id)
                .await?
                .ok_or(WorkflowStoreError::NotFound)?;
            transaction
                .commit()
                .await
                .map_err(|_| WorkflowStoreError::Database(
                    "commit_idempotent_agent_job_transition",
                ))?;
            return Ok(job);
        }

        let current = sqlx::query(
            "SELECT state, version, lease_owner, claim_token, \
                    CASE WHEN lease_expires_at IS NULL THEN NULL \
                         ELSE (extract(epoch FROM lease_expires_at) * 1000)::bigint END \
                         AS lease_expires_at_unix_ms \
               FROM workflow_instances WHERE workflow_id = $1 FOR UPDATE",
        )
        .bind(&draft.job_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_job_transition_state"))?
        .ok_or(WorkflowStoreError::NotFound)?;
        let current_state = WorkflowState::parse(current.get::<String, _>("state").as_str())?;
        let current_version: i64 = current.get("version");
        let lease_expires_at_unix_ms: Option<i64> = current.get("lease_expires_at_unix_ms");
        if current_version != draft.expected_version {
            return Err(WorkflowStoreError::VersionConflict {
                expected: draft.expected_version,
                actual: current_version,
            });
        }
        if current_state != draft.from_state
            || !current_state.can_transition_to(draft.to_state)
            || current.get::<Option<String>, _>("lease_owner").as_deref()
                != Some(draft.claim_owner.as_str())
            || current.get::<Option<String>, _>("claim_token").as_deref()
                != Some(draft.claim_token.as_str())
            || lease_expires_at_unix_ms.is_none_or(|expires| expires <= draft.now_unix_ms)
        {
            return Err(WorkflowStoreError::StateConflict);
        }
        let workflow_version = current_version + 1;
        sqlx::query(
            r#"
            INSERT INTO workflow_transitions (
                workflow_id, from_state, to_state, workflow_version,
                idempotency_key, correlation_id, causation_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&draft.job_id)
        .bind(draft.from_state.as_str())
        .bind(draft.to_state.as_str())
        .bind(workflow_version)
        .bind(&draft.idempotency_key)
        .bind(&draft.correlation_id)
        .bind(&draft.causation_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("insert_agent_job_transition"))?;
        let release_lease = draft.to_state.releases_lease();
        let updated = sqlx::query(
            r#"
            UPDATE workflow_instances
               SET state = $2,
                   version = $3,
                   wake_at = CASE WHEN $4::bigint IS NULL THEN wake_at
                                  ELSE to_timestamp($4::bigint / 1000.0) END,
                   next_attempt_at =
                       CASE WHEN $4::bigint IS NULL THEN NULL
                            ELSE to_timestamp($4::bigint / 1000.0) END,
                   lease_owner = CASE WHEN $5 THEN NULL ELSE lease_owner END,
                   claim_token = CASE WHEN $5 THEN NULL ELSE claim_token END,
                   lease_expires_at =
                       CASE WHEN $5 THEN NULL ELSE lease_expires_at END,
                   heartbeat_at = CASE WHEN $5 THEN NULL ELSE heartbeat_at END,
                   updated_at = now()
             WHERE workflow_id = $1 AND version = $6 AND state = $7
            "#,
        )
        .bind(&draft.job_id)
        .bind(draft.to_state.as_str())
        .bind(workflow_version)
        .bind(draft.next_attempt_at_unix_ms)
        .bind(release_lease)
        .bind(draft.expected_version)
        .bind(draft.from_state.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("update_agent_job_workflow"))?;
        if updated.rows_affected() != 1 {
            return Err(WorkflowStoreError::StateConflict);
        }
        sqlx::query(
            r#"
            UPDATE agent_jobs
               SET resume_state =
                       CASE WHEN $6 = 'RETRYABLE_FAILED' THEN $7
                            ELSE NULL END,
                   decision_json =
                       CASE WHEN $6 IN ('COMPLETED', 'TERMINAL_FAILED') THEN NULL
                            ELSE COALESCE($2::jsonb, decision_json) END,
                   tool_result_json =
                       CASE WHEN $6 IN ('COMPLETED', 'TERMINAL_FAILED') THEN NULL
                            ELSE COALESCE($3::jsonb, tool_result_json) END,
                   linked_event_sequences =
                       COALESCE($4::bigint[], linked_event_sequences),
                   error_code = $5,
                   updated_at = now()
             WHERE job_id = $1
            "#,
        )
        .bind(&draft.job_id)
        .bind(decision_json)
        .bind(tool_result_json)
        .bind(draft.linked_event_sequences.as_deref())
        .bind(&draft.error_code)
        .bind(draft.to_state.as_str())
        .bind(draft.from_state.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("update_agent_job"))?;
        let job = load_agent_job_in_transaction(&mut transaction, &draft.job_id)
            .await?
            .ok_or(WorkflowStoreError::NotFound)?;
        transaction
            .commit()
            .await
            .map_err(|_| WorkflowStoreError::Database("commit_agent_job_transition"))?;
        Ok(job)
    }

    pub async fn heartbeat_agent_job(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<bool, WorkflowStoreError> {
        if now_unix_ms < 0 || lease_duration_ms <= 0 {
            return Err(WorkflowStoreError::Validation(
                "invalid_agent_job_heartbeat",
            ));
        }
        let result = sqlx::query(
            r#"
            UPDATE workflow_instances
               SET heartbeat_at = to_timestamp($4::bigint / 1000.0),
                   lease_expires_at =
                       to_timestamp(($4::bigint + $5::bigint) / 1000.0),
                   updated_at = now()
             WHERE workflow_id = $1
               AND lease_owner = $2
               AND claim_token = $3
               AND state IN (
                   'CLAIMED', 'AGENT_RUNNING', 'AWAITING_TOOL', 'COMMITTING'
               )
               AND lease_expires_at > to_timestamp($4::bigint / 1000.0)
            "#,
        )
        .bind(job_id)
        .bind(claim_owner)
        .bind(claim_token)
        .bind(now_unix_ms)
        .bind(lease_duration_ms)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("heartbeat_agent_job"))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn request_agent_job_cancellation(
        &self,
        job_id: &str,
        now_unix_ms: i64,
    ) -> Result<bool, WorkflowStoreError> {
        let result = sqlx::query(
            "UPDATE agent_jobs \
                SET cancellation_requested_at = to_timestamp($2::bigint / 1000.0) \
              WHERE job_id = $1 AND cancellation_requested_at IS NULL",
        )
        .bind(job_id)
        .bind(now_unix_ms)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("cancel_agent_job"))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn agent_job_cancellation_requested(
        &self,
        job_id: &str,
    ) -> Result<bool, WorkflowStoreError> {
        sqlx::query_scalar(
            "SELECT cancellation_requested_at IS NOT NULL \
               FROM agent_jobs WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_job_cancellation"))?
        .ok_or(WorkflowStoreError::NotFound)
    }

    pub async fn append_agent_job_evidence(
        &self,
        draft: &AgentJobEvidenceDraft,
    ) -> Result<(), WorkflowStoreError> {
        validate_agent_job_evidence(draft)?;
        let result = sqlx::query(
            r#"
            INSERT INTO agent_job_evidence (
                job_id, attempt, phase, model_id, runtime_version,
                prompt_template_hash, tool_schema_hash, retrieval_hash,
                input_hash, output_hash, input_tokens, output_tokens,
                latency_ms, tool_call_count, linked_event_sequences,
                visibility_label, retention_until
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16,
                to_timestamp($17::bigint / 1000.0)
            )
            ON CONFLICT (job_id, attempt, phase) DO NOTHING
            "#,
        )
        .bind(&draft.job_id)
        .bind(draft.attempt)
        .bind(&draft.phase)
        .bind(&draft.model_id)
        .bind(&draft.runtime_version)
        .bind(&draft.prompt_template_hash)
        .bind(&draft.tool_schema_hash)
        .bind(&draft.retrieval_hash)
        .bind(&draft.input_hash)
        .bind(&draft.output_hash)
        .bind(draft.input_tokens)
        .bind(draft.output_tokens)
        .bind(draft.latency_ms)
        .bind(draft.tool_call_count)
        .bind(&draft.linked_event_sequences)
        .bind(&draft.visibility_label)
        .bind(draft.retention_until_unix_ms)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("append_agent_job_evidence"))?;
        if result.rows_affected() == 0 {
            let exact: bool = sqlx::query_scalar(
                r#"
                SELECT model_id = $4
                   AND runtime_version = $5
                   AND prompt_template_hash = $6
                   AND tool_schema_hash = $7
                   AND retrieval_hash = $8
                   AND input_hash = $9
                   AND output_hash = $10
                   AND input_tokens = $11
                   AND output_tokens = $12
                   AND latency_ms = $13
                   AND tool_call_count = $14
                   AND linked_event_sequences = $15
                   AND visibility_label = $16
                  FROM agent_job_evidence
                 WHERE job_id = $1 AND attempt = $2 AND phase = $3
                "#,
            )
            .bind(&draft.job_id)
            .bind(draft.attempt)
            .bind(&draft.phase)
            .bind(&draft.model_id)
            .bind(&draft.runtime_version)
            .bind(&draft.prompt_template_hash)
            .bind(&draft.tool_schema_hash)
            .bind(&draft.retrieval_hash)
            .bind(&draft.input_hash)
            .bind(&draft.output_hash)
            .bind(draft.input_tokens)
            .bind(draft.output_tokens)
            .bind(draft.latency_ms)
            .bind(draft.tool_call_count)
            .bind(&draft.linked_event_sequences)
            .bind(&draft.visibility_label)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| WorkflowStoreError::Database("verify_agent_job_evidence"))?;
            if !exact {
                return Err(WorkflowStoreError::IdempotencyConflict);
            }
        }
        Ok(())
    }

}
