#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobEnqueueDraft {
    pub job_id: String,
    pub campaign_id: String,
    pub actor_id: String,
    pub agent_kind: String,
    pub authority_contract_id: String,
    pub authority_mode: String,
    pub authority_contract_version: i64,
    pub input_event_sequence: i64,
    pub input_stream_version: i64,
    pub visibility_scope_json: String,
    pub rag_snapshot_id: String,
    pub provider_id: String,
    pub provider_type: String,
    pub model_id: String,
    pub model_artifact_sha256: String,
    pub route_authorization_event_id: String,
    pub prompt_template_id: String,
    pub prompt_template_version: String,
    pub tool_schema_version: String,
    pub idempotency_key: String,
    pub deadline_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentJob {
    pub job_id: String,
    pub campaign_id: String,
    pub actor_id: String,
    pub agent_kind: String,
    pub authority_contract_id: String,
    pub authority_mode: String,
    pub authority_contract_version: i64,
    pub input_event_sequence: i64,
    pub input_stream_id: String,
    pub input_stream_version: i64,
    pub visibility_scope_json: String,
    pub rag_snapshot_id: String,
    pub provider_id: String,
    pub provider_type: String,
    pub model_id: String,
    pub model_artifact_sha256: String,
    pub route_authorization_event_id: String,
    pub prompt_template_id: String,
    pub prompt_template_version: String,
    pub tool_schema_version: String,
    pub idempotency_key: String,
    pub deadline_unix_ms: i64,
    pub state: WorkflowState,
    pub resume_state: Option<WorkflowState>,
    pub version: i64,
    pub claim_owner: Option<String>,
    pub claim_token: Option<String>,
    pub lease_expires_at_unix_ms: Option<i64>,
    pub heartbeat_at_unix_ms: Option<i64>,
    pub attempt: i32,
    pub next_attempt_at_unix_ms: Option<i64>,
    pub decision_json: Option<String>,
    pub tool_result_json: Option<String>,
    pub linked_event_sequences: Vec<i64>,
    pub cancellation_requested_at_unix_ms: Option<i64>,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobTransitionDraft {
    pub job_id: String,
    pub claim_owner: String,
    pub claim_token: String,
    pub expected_version: i64,
    pub from_state: WorkflowState,
    pub to_state: WorkflowState,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub decision_json: Option<String>,
    pub tool_result_json: Option<String>,
    pub linked_event_sequences: Option<Vec<i64>>,
    pub error_code: Option<String>,
    pub next_attempt_at_unix_ms: Option<i64>,
    pub now_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobEvidenceDraft {
    pub job_id: String,
    pub attempt: i32,
    pub phase: String,
    pub model_id: String,
    pub runtime_version: String,
    pub prompt_template_hash: String,
    pub tool_schema_hash: String,
    pub retrieval_hash: String,
    pub input_hash: String,
    pub output_hash: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub latency_ms: i64,
    pub tool_call_count: i32,
    pub linked_event_sequences: Vec<i64>,
    pub visibility_label: String,
    pub retention_until_unix_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentContextChunk {
    pub chunk_id: String,
    pub source_event_sequence: i64,
    pub visibility_label: String,
    pub visibility_subject: Option<String>,
    pub fact_provenance_json: String,
    pub chunk_hash: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentContextSnapshot {
    pub input_payload_json: String,
    pub chunks: Vec<DurableAgentContextChunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentAuthoritySnapshot {
    pub contract_id: String,
    pub campaign_id: String,
    pub authority_mode: String,
    pub authority_owner: String,
    pub contract_version: i64,
    pub prompt_version: String,
    pub agent_pack_version: String,
    pub tool_schema_version: String,
    pub model_route_snapshot: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableAgentApproval {
    pub approval_id: String,
    pub approval_event_sequence: i64,
    pub approved_by: String,
    pub idempotency_key: String,
}

impl DurableWorkflowStore {
    pub async fn check_agent_job_readiness(&self) -> Result<(), WorkflowStoreError> {
        let ready: bool = sqlx::query_scalar(
            "SELECT to_regclass('agent_jobs') IS NOT NULL \
                 AND to_regclass('agent_job_evidence') IS NOT NULL \
                 AND to_regclass('agent_job_approvals') IS NOT NULL \
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
}

fn validate_agent_job_enqueue(
    draft: &AgentJobEnqueueDraft,
) -> Result<(), WorkflowStoreError> {
    for (value, reason) in [
        (&draft.job_id, "job_id_required"),
        (&draft.campaign_id, "campaign_id_required"),
        (&draft.actor_id, "actor_id_required"),
        (&draft.agent_kind, "agent_kind_required"),
        (&draft.authority_contract_id, "authority_contract_id_required"),
        (&draft.authority_mode, "authority_mode_required"),
        (&draft.rag_snapshot_id, "rag_snapshot_id_required"),
        (&draft.provider_id, "provider_id_required"),
        (&draft.provider_type, "provider_type_required"),
        (&draft.model_id, "model_id_required"),
        (
            &draft.route_authorization_event_id,
            "route_authorization_event_id_required",
        ),
        (&draft.prompt_template_id, "prompt_template_id_required"),
        (
            &draft.prompt_template_version,
            "prompt_template_version_required",
        ),
        (&draft.tool_schema_version, "tool_schema_version_required"),
        (&draft.idempotency_key, "idempotency_key_required"),
    ] {
        validate_identifier(value, reason)?;
    }
    if draft.authority_contract_version <= 0
        || draft.input_event_sequence <= 0
        || draft.input_stream_version < 0
        || draft.deadline_unix_ms <= 0
        || !matches!(
            draft.authority_mode.as_str(),
            "AI_KP" | "HUMAN_KP"
        )
        || !matches!(
            draft.provider_type.as_str(),
            "cloud" | "ollama" | "llama_cpp"
        )
        || !matches!(
            draft.agent_kind.as_str(),
            "ai_keeper_orchestrator" | "keeper_copilot"
        )
        || !valid_labelled_sha256(&draft.model_artifact_sha256)
    {
        return Err(WorkflowStoreError::Validation(
            "invalid_agent_job_binding",
        ));
    }
    Ok(())
}

fn validate_visibility_scope(scope: &Value) -> Result<(), WorkflowStoreError> {
    let Some(scope) = scope.as_object() else {
        return Err(WorkflowStoreError::Validation(
            "invalid_visibility_scope",
        ));
    };
    let allowed_labels = scope
        .get("allowed_labels")
        .and_then(Value::as_array)
        .filter(|labels| !labels.is_empty() && labels.len() <= 11)
        .ok_or(WorkflowStoreError::Validation(
            "invalid_visibility_scope",
        ))?;
    let output_label = scope
        .get("output_label")
        .and_then(Value::as_str)
        .filter(|label| !label.trim().is_empty())
        .ok_or(WorkflowStoreError::Validation(
            "invalid_visibility_scope",
        ))?;
    if scope.len() != 3
        || !["allowed_labels", "output_label", "subject_id"]
            .iter()
            .all(|field| scope.contains_key(*field))
        || allowed_labels.iter()
        .any(|label| label.as_str().is_none_or(|label| label.trim().is_empty()))
        || !allowed_labels
            .iter()
            .any(|label| label.as_str() == Some(output_label))
        || !scope.contains_key("subject_id")
        || scope
            .get("subject_id")
            .is_some_and(|subject| !subject.is_null() && subject.as_str().is_none())
        || scope
            .get("subject_id")
            .and_then(Value::as_str)
            .is_some_and(|subject| subject.trim().is_empty())
    {
        return Err(WorkflowStoreError::Validation(
            "invalid_visibility_scope",
        ));
    }
    Ok(())
}

fn validate_agent_job_transition(
    draft: &AgentJobTransitionDraft,
) -> Result<(), WorkflowStoreError> {
    for (value, reason) in [
        (&draft.job_id, "job_id_required"),
        (&draft.claim_owner, "claim_owner_required"),
        (&draft.claim_token, "claim_token_required"),
        (&draft.idempotency_key, "idempotency_key_required"),
        (&draft.correlation_id, "correlation_id_required"),
        (&draft.causation_id, "causation_id_required"),
    ] {
        validate_identifier(value, reason)?;
    }
    if draft.expected_version < 0
        || draft.now_unix_ms < 0
        || !draft.from_state.can_transition_to(draft.to_state)
    {
        return Err(WorkflowStoreError::Validation(
            "invalid_agent_job_transition",
        ));
    }
    validate_timestamp(draft.next_attempt_at_unix_ms)
}

fn validate_agent_job_evidence(
    draft: &AgentJobEvidenceDraft,
) -> Result<(), WorkflowStoreError> {
    if draft.attempt <= 0
        || draft.input_tokens < 0
        || draft.output_tokens < 0
        || draft.latency_ms < 0
        || draft.tool_call_count < 0
        || draft.retention_until_unix_ms <= 0
        || !matches!(
            draft.phase.as_str(),
            "authority"
                | "context"
                | "provider"
                | "tool"
                | "canonical_commit"
                | "completed"
                | "failed"
        )
        || [
            &draft.prompt_template_hash,
            &draft.tool_schema_hash,
            &draft.retrieval_hash,
            &draft.input_hash,
            &draft.output_hash,
        ]
        .iter()
        .any(|hash| !valid_labelled_sha256(hash))
    {
        return Err(WorkflowStoreError::Validation(
            "invalid_agent_job_evidence",
        ));
    }
    Ok(())
}

fn valid_plain_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_labelled_sha256(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(valid_plain_sha256)
}

fn agent_job_binding_matches(
    existing: &DurableAgentJob,
    draft: &AgentJobEnqueueDraft,
    visibility_scope_json: &str,
) -> bool {
    existing.job_id == draft.job_id
        && existing.campaign_id == draft.campaign_id
        && existing.actor_id == draft.actor_id
        && existing.agent_kind == draft.agent_kind
        && existing.authority_contract_id == draft.authority_contract_id
        && existing.authority_mode == draft.authority_mode
        && existing.authority_contract_version == draft.authority_contract_version
        && existing.input_event_sequence == draft.input_event_sequence
        && existing.input_stream_version == draft.input_stream_version
        && json_values_equal(
            &existing.visibility_scope_json,
            visibility_scope_json,
        )
        && existing.rag_snapshot_id == draft.rag_snapshot_id
        && existing.provider_id == draft.provider_id
        && existing.provider_type == draft.provider_type
        && existing.model_id == draft.model_id
        && existing.model_artifact_sha256 == draft.model_artifact_sha256
        && existing.route_authorization_event_id == draft.route_authorization_event_id
        && existing.prompt_template_id == draft.prompt_template_id
        && existing.prompt_template_version == draft.prompt_template_version
        && existing.tool_schema_version == draft.tool_schema_version
        && existing.idempotency_key == draft.idempotency_key
        && existing.deadline_unix_ms == draft.deadline_unix_ms
}

fn json_values_equal(left: &str, right: &str) -> bool {
    serde_json::from_str::<Value>(left)
        .ok()
        .zip(serde_json::from_str::<Value>(right).ok())
        .is_some_and(|(left, right)| left == right)
}

fn agent_job_select_sql() -> &'static str {
    r#"
    SELECT job.job_id, job.campaign_id, job.actor_id, job.agent_kind,
           job.authority_contract_id, job.authority_mode,
           job.authority_contract_version, job.input_event_sequence,
           source.stream_id AS input_stream_id,
           job.input_stream_version, job.visibility_scope::text
               AS visibility_scope_json,
           job.rag_snapshot_id, job.provider_id, job.provider_type,
           job.model_id, job.model_artifact_sha256,
           job.route_authorization_event_id, job.prompt_template_id,
           job.prompt_template_version, job.tool_schema_version,
           job.idempotency_key,
           (extract(epoch FROM job.deadline_at) * 1000)::bigint
               AS deadline_unix_ms,
           workflow.state, job.resume_state, workflow.version,
           workflow.lease_owner AS claim_owner, workflow.claim_token,
           CASE WHEN workflow.lease_expires_at IS NULL THEN NULL
                ELSE (extract(epoch FROM workflow.lease_expires_at) * 1000)::bigint
                END AS lease_expires_at_unix_ms,
           CASE WHEN workflow.heartbeat_at IS NULL THEN NULL
                ELSE (extract(epoch FROM workflow.heartbeat_at) * 1000)::bigint
                END AS heartbeat_at_unix_ms,
           workflow.attempt,
           CASE WHEN workflow.next_attempt_at IS NULL THEN NULL
                ELSE (extract(epoch FROM workflow.next_attempt_at) * 1000)::bigint
                END AS next_attempt_at_unix_ms,
           job.decision_json::text AS decision_json,
           job.tool_result_json::text AS tool_result_json,
           job.linked_event_sequences,
           CASE WHEN job.cancellation_requested_at IS NULL THEN NULL
                ELSE (extract(epoch FROM job.cancellation_requested_at) * 1000)::bigint
                END AS cancellation_requested_at_unix_ms,
           job.error_code
      FROM agent_jobs AS job
      JOIN workflow_instances AS workflow ON workflow.workflow_id = job.job_id
      JOIN event_store AS source ON source.sequence = job.input_event_sequence
     WHERE job.job_id = $1
    "#
}

async fn load_agent_job_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    job_id: &str,
) -> Result<Option<DurableAgentJob>, WorkflowStoreError> {
    let row = sqlx::query(agent_job_select_sql())
        .bind(job_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_job_transaction"))?;
    row.as_ref().map(agent_job_from_row).transpose()
}

fn agent_job_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<DurableAgentJob, WorkflowStoreError> {
    let resume_state = row
        .get::<Option<String>, _>("resume_state")
        .as_deref()
        .map(WorkflowState::parse)
        .transpose()?;
    Ok(DurableAgentJob {
        job_id: row.get("job_id"),
        campaign_id: row.get("campaign_id"),
        actor_id: row.get("actor_id"),
        agent_kind: row.get("agent_kind"),
        authority_contract_id: row.get("authority_contract_id"),
        authority_mode: row.get("authority_mode"),
        authority_contract_version: row.get("authority_contract_version"),
        input_event_sequence: row.get("input_event_sequence"),
        input_stream_id: row.get("input_stream_id"),
        input_stream_version: row.get("input_stream_version"),
        visibility_scope_json: row.get("visibility_scope_json"),
        rag_snapshot_id: row.get("rag_snapshot_id"),
        provider_id: row.get("provider_id"),
        provider_type: row.get("provider_type"),
        model_id: row.get("model_id"),
        model_artifact_sha256: row.get("model_artifact_sha256"),
        route_authorization_event_id: row.get("route_authorization_event_id"),
        prompt_template_id: row.get("prompt_template_id"),
        prompt_template_version: row.get("prompt_template_version"),
        tool_schema_version: row.get("tool_schema_version"),
        idempotency_key: row.get("idempotency_key"),
        deadline_unix_ms: row.get("deadline_unix_ms"),
        state: WorkflowState::parse(row.get::<String, _>("state").as_str())?,
        resume_state,
        version: row.get("version"),
        claim_owner: row.get("claim_owner"),
        claim_token: row.get("claim_token"),
        lease_expires_at_unix_ms: row.get("lease_expires_at_unix_ms"),
        heartbeat_at_unix_ms: row.get("heartbeat_at_unix_ms"),
        attempt: row.get("attempt"),
        next_attempt_at_unix_ms: row.get("next_attempt_at_unix_ms"),
        decision_json: row.get("decision_json"),
        tool_result_json: row.get("tool_result_json"),
        linked_event_sequences: row.get("linked_event_sequences"),
        cancellation_requested_at_unix_ms: row.get("cancellation_requested_at_unix_ms"),
        error_code: row.get("error_code"),
    })
}
