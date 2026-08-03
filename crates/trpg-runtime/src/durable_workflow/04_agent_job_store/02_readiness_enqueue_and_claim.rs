impl DurableWorkflowStore {
    pub async fn check_agent_job_readiness(&self) -> Result<(), WorkflowStoreError> {
        let ready: bool = sqlx::query_scalar(
            "SELECT to_regclass('agent_jobs') IS NOT NULL \
                 AND to_regclass('agent_job_evidence') IS NOT NULL \
                 AND to_regclass('agent_job_approvals') IS NOT NULL \
                 AND to_regclass('agent_job_tool_receipts') IS NOT NULL \
                 AND EXISTS ( \
                     SELECT 1 FROM information_schema.columns \
                      WHERE table_schema = 'public' \
                        AND table_name = 'workflow_instances' \
                        AND column_name = 'claim_token' \
                 )",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("agent_job_readiness"))?;
        if ready {
            Ok(())
        } else {
            Err(WorkflowStoreError::Migration)
        }
    }

    pub async fn enqueue_agent_job(
        &self,
        draft: &AgentJobEnqueueDraft,
    ) -> Result<DurableAgentJob, WorkflowStoreError> {
        validate_agent_job_enqueue(draft)?;
        let visibility_scope_json = normalize_json(&draft.visibility_scope_json)?;
        let visibility_scope: Value = serde_json::from_str(&visibility_scope_json)
            .map_err(|_| WorkflowStoreError::Validation("invalid_visibility_scope"))?;
        validate_visibility_scope(&visibility_scope)?;
        let input_json = serde_json::json!({
            "input_event_sequence": draft.input_event_sequence,
            "input_stream_version": draft.input_stream_version,
        })
        .to_string();

        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkflowStoreError::Database("begin_agent_job_enqueue"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("{}:{}", draft.campaign_id, draft.idempotency_key))
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkflowStoreError::Database("lock_agent_job_enqueue"))?;

        if let Some(existing_id) = sqlx::query_scalar::<_, String>(
            "SELECT job_id FROM agent_jobs \
             WHERE job_id = $1 OR (campaign_id = $2 AND idempotency_key = $3) \
             ORDER BY CASE WHEN job_id = $1 THEN 0 ELSE 1 END LIMIT 1",
        )
        .bind(&draft.job_id)
        .bind(&draft.campaign_id)
        .bind(&draft.idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_idempotent_agent_job"))?
        {
            let existing = load_agent_job_in_transaction(&mut transaction, &existing_id)
                .await?
                .ok_or(WorkflowStoreError::IntegrityViolation(
                    "agent_job_workflow_missing",
                ))?;
            if !agent_job_binding_matches(&existing, draft, &visibility_scope_json) {
                return Err(WorkflowStoreError::IdempotencyConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| WorkflowStoreError::Database("commit_agent_job_enqueue"))?;
            return Ok(existing);
        }

        sqlx::query(
            r#"
            INSERT INTO workflow_instances (
                workflow_id, campaign_id, workflow_type, state, version,
                input_json, wake_at, next_attempt_at
            ) VALUES ($1, $2, 'agent_job', 'REQUESTED', 0, $3,
                      now(), now())
            "#,
        )
        .bind(&draft.job_id)
        .bind(&draft.campaign_id)
        .bind(input_json)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("insert_agent_job_workflow"))?;

        sqlx::query(
            r#"
            INSERT INTO agent_jobs (
                job_id, campaign_id, actor_id, agent_kind,
                authority_contract_id, authority_mode, authority_contract_version,
                input_event_sequence, input_stream_version, visibility_scope,
                rag_snapshot_id, provider_id, provider_type, model_id,
                model_artifact_sha256, route_authorization_event_id,
                prompt_template_id, prompt_template_version, tool_schema_version,
                idempotency_key, deadline_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10::jsonb,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                to_timestamp($21::bigint / 1000.0)
            )
            "#,
        )
        .bind(&draft.job_id)
        .bind(&draft.campaign_id)
        .bind(&draft.actor_id)
        .bind(&draft.agent_kind)
        .bind(&draft.authority_contract_id)
        .bind(&draft.authority_mode)
        .bind(draft.authority_contract_version)
        .bind(draft.input_event_sequence)
        .bind(draft.input_stream_version)
        .bind(&visibility_scope_json)
        .bind(&draft.rag_snapshot_id)
        .bind(&draft.provider_id)
        .bind(&draft.provider_type)
        .bind(&draft.model_id)
        .bind(&draft.model_artifact_sha256)
        .bind(&draft.route_authorization_event_id)
        .bind(&draft.prompt_template_id)
        .bind(&draft.prompt_template_version)
        .bind(&draft.tool_schema_version)
        .bind(&draft.idempotency_key)
        .bind(draft.deadline_unix_ms)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("insert_agent_job"))?;

        let job = load_agent_job_in_transaction(&mut transaction, &draft.job_id)
            .await?
            .ok_or(WorkflowStoreError::IntegrityViolation(
                "inserted_agent_job_missing",
            ))?;
        transaction
            .commit()
            .await
            .map_err(|_| WorkflowStoreError::Database("commit_agent_job_enqueue"))?;
        Ok(job)
    }

    pub async fn load_agent_job(
        &self,
        job_id: &str,
    ) -> Result<Option<DurableAgentJob>, WorkflowStoreError> {
        validate_identifier(job_id, "job_id_required")?;
        let row = sqlx::query(agent_job_select_sql())
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| WorkflowStoreError::Database("load_agent_job"))?;
        row.as_ref().map(agent_job_from_row).transpose()
    }

    pub async fn claim_due_agent_job(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<Option<DurableAgentJob>, WorkflowStoreError> {
        validate_identifier(claim_owner, "claim_owner_required")?;
        if now_unix_ms < 0 || lease_duration_ms <= 0 {
            return Err(WorkflowStoreError::Validation(
                "invalid_agent_job_lease",
            ));
        }
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkflowStoreError::Database("begin_agent_job_claim"))?;
        let candidate = sqlx::query(
            r#"
            SELECT workflow.workflow_id, workflow.state,
                   workflow.version, workflow.attempt
              FROM workflow_instances AS workflow
              JOIN agent_jobs AS job ON job.job_id = workflow.workflow_id
             WHERE (
                    workflow.state IN ('REQUESTED', 'RETRYABLE_FAILED')
                    AND (
                        workflow.next_attempt_at IS NULL
                        OR workflow.next_attempt_at
                           <= to_timestamp($1::bigint / 1000.0)
                    )
                   )
                OR (
                    workflow.state IN (
                        'CLAIMED', 'AGENT_RUNNING', 'COMMITTING'
                    )
                    AND (
                        workflow.lease_expires_at IS NULL
                        OR workflow.lease_expires_at
                           <= to_timestamp($1::bigint / 1000.0)
                    )
                   )
                OR (
                    workflow.state = 'AWAITING_TOOL'
                    AND (
                        (
                            job.authority_mode = 'AI_KP'
                            AND (
                                workflow.lease_expires_at IS NULL
                                OR workflow.lease_expires_at
                                   <= to_timestamp($1::bigint / 1000.0)
                            )
                        )
                        OR EXISTS (
                            SELECT 1 FROM agent_job_approvals AS approval
                             WHERE approval.job_id = job.job_id
                        )
                        OR job.cancellation_requested_at IS NOT NULL
                        OR job.deadline_at
                           <= to_timestamp($1::bigint / 1000.0)
                    )
                   )
             ORDER BY COALESCE(
                         workflow.next_attempt_at,
                         workflow.wake_at,
                         to_timestamp(0)
                      ),
                      workflow.workflow_id
             FOR UPDATE OF workflow, job SKIP LOCKED
             LIMIT 1
            "#,
        )
        .bind(now_unix_ms)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("select_due_agent_job"))?;
        let Some(candidate) = candidate else {
            transaction
                .commit()
                .await
                .map_err(|_| WorkflowStoreError::Database("commit_empty_agent_job_claim"))?;
            return Ok(None);
        };
        let job_id: String = candidate.get("workflow_id");
        let resume_state = WorkflowState::parse(candidate.get::<String, _>("state").as_str())?;
        let previous_version: i64 = candidate.get("version");
        let previous_attempt: i32 = candidate.get("attempt");
        let claim_token: String = sqlx::query_scalar("SELECT gen_random_uuid()::text")
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| WorkflowStoreError::Database("create_agent_job_claim_token"))?;

        let updated = sqlx::query(
            r#"
            UPDATE workflow_instances
               SET state = 'CLAIMED',
                   version = version + 1,
                   lease_owner = $2,
                   claim_token = $3,
                   lease_expires_at =
                       to_timestamp(($4::bigint + $5::bigint) / 1000.0),
                   heartbeat_at = to_timestamp($4::bigint / 1000.0),
                   attempt = attempt + 1,
                   updated_at = now()
             WHERE workflow_id = $1
            "#,
        )
        .bind(&job_id)
        .bind(claim_owner)
        .bind(&claim_token)
        .bind(now_unix_ms)
        .bind(lease_duration_ms)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("claim_agent_job"))?;
        if updated.rows_affected() != 1 {
            return Err(WorkflowStoreError::StateConflict);
        }
        sqlx::query(
            r#"
            INSERT INTO workflow_transitions (
                workflow_id, from_state, to_state, workflow_version,
                idempotency_key, correlation_id, causation_id
            ) VALUES ($1, $2, 'CLAIMED', $3, $4, $1, $1)
            "#,
        )
        .bind(&job_id)
        .bind(resume_state.as_str())
        .bind(previous_version + 1)
        .bind(format!(
            "agent-job-claim:{}:{}",
            job_id,
            previous_attempt + 1
        ))
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("insert_agent_job_claim_transition"))?;
        sqlx::query(
            "UPDATE agent_jobs \
                SET resume_state = CASE \
                        WHEN $2 = 'RETRYABLE_FAILED' THEN resume_state \
                        ELSE $2 \
                    END, \
                    updated_at = now() \
              WHERE job_id = $1",
        )
        .bind(&job_id)
        .bind(resume_state.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("record_agent_job_resume_state"))?;
        let job = load_agent_job_in_transaction(&mut transaction, &job_id)
            .await?
            .ok_or(WorkflowStoreError::IntegrityViolation(
                "claimed_agent_job_missing",
            ))?;
        transaction
            .commit()
            .await
            .map_err(|_| WorkflowStoreError::Database("commit_agent_job_claim"))?;
        Ok(Some(job))
    }

}
