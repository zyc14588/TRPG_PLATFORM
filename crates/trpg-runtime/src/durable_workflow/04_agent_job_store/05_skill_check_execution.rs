impl DurableWorkflowStore {
    pub async fn execute_agent_job_skill_check<F>(
        &self,
        draft: &AgentJobSkillCheckDraft,
        generate_roll: F,
    ) -> Result<DurableAgentJobToolReceipt, WorkflowStoreError>
    where
        F: FnOnce(u8) -> Result<AgentJobSkillCheckRollDraft, WorkflowStoreError> + Send,
    {
        for (value, reason) in [
            (&draft.job_id, "job_id_required"),
            (&draft.claim_owner, "claim_owner_required"),
            (&draft.claim_token, "claim_token_required"),
            (&draft.idempotency_key, "tool_idempotency_key_required"),
            (&draft.character_id, "character_id_required"),
        ] {
            validate_identifier(value, reason)?;
        }
        if draft.expected_attempt <= 0
            || draft.now_unix_ms < 0
            || draft.adjustment != "NONE"
            || draft.skill_name.trim().is_empty()
            || draft.skill_name.len() > 128
            || draft.skill_name.chars().any(char::is_control)
        {
            return Err(WorkflowStoreError::Validation(
                "invalid_agent_skill_check",
            ));
        }
        let request_json = normalize_json(
            &serde_json::json!({
                "adjustment": draft.adjustment,
                "character_id": draft.character_id,
                "skill_name": draft.skill_name,
            })
            .to_string(),
        )?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkflowStoreError::Database("begin_agent_tool_receipt"))?;
        let job = sqlx::query(
            r#"
            SELECT job.campaign_id, job.authority_mode, job.agent_kind,
                   job.idempotency_key AS job_idempotency_key,
                   workflow.state, workflow.attempt, workflow.lease_owner,
                   workflow.claim_token,
                   CASE WHEN workflow.lease_expires_at IS NULL THEN NULL
                        ELSE (
                            extract(epoch FROM workflow.lease_expires_at) * 1000
                        )::bigint
                   END AS lease_expires_at_unix_ms
              FROM agent_jobs AS job
              JOIN workflow_instances AS workflow
                ON workflow.workflow_id = job.job_id
             WHERE job.job_id = $1
             FOR UPDATE OF job, workflow
            "#,
        )
        .bind(&draft.job_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_tool_job"))?
        .ok_or(WorkflowStoreError::NotFound)?;
        let job_idempotency_key: String = job.get("job_idempotency_key");
        let lease_expires_at_unix_ms: Option<i64> = job.get("lease_expires_at_unix_ms");
        if job.get::<String, _>("authority_mode") != "AI_KP"
            || job.get::<String, _>("agent_kind") != "ai_keeper_orchestrator"
            || job.get::<String, _>("state") != "AWAITING_TOOL"
            || job.get::<i32, _>("attempt") != draft.expected_attempt
            || job.get::<Option<String>, _>("lease_owner").as_deref()
                != Some(draft.claim_owner.as_str())
            || job.get::<Option<String>, _>("claim_token").as_deref()
                != Some(draft.claim_token.as_str())
            || lease_expires_at_unix_ms.is_none_or(|expires| expires <= draft.now_unix_ms)
            || draft.idempotency_key != format!("{job_idempotency_key}:tool")
        {
            return Err(WorkflowStoreError::StateConflict);
        }

        if let Some(existing) = sqlx::query(
            r#"
            SELECT tool_name, request_json::text AS request_json,
                   execution_id, result_json::text AS result_json, result_hash
              FROM agent_job_tool_receipts
             WHERE job_id = $1 AND idempotency_key = $2
            "#,
        )
        .bind(&draft.job_id)
        .bind(&draft.idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_tool_receipt"))?
        {
            let existing_request: String = existing.get("request_json");
            if existing.get::<String, _>("tool_name") != "request_skill_check"
                || !json_values_equal(&existing_request, &request_json)
            {
                return Err(WorkflowStoreError::IdempotencyConflict);
            }
            let receipt = DurableAgentJobToolReceipt {
                execution_id: existing.get("execution_id"),
                result_json: normalize_json(&existing.get::<String, _>("result_json"))?,
                result_hash: existing.get("result_hash"),
            };
            if receipt.result_hash
                != labelled_sha256(receipt.result_json.as_bytes())
            {
                return Err(WorkflowStoreError::IntegrityViolation(
                    "agent_tool_receipt_hash_mismatch",
                ));
            }
            transaction
                .commit()
                .await
                .map_err(|_| WorkflowStoreError::Database("commit_agent_tool_replay"))?;
            return Ok(receipt);
        }

        let campaign_id: String = job.get("campaign_id");
        let target: Option<i32> = sqlx::query_scalar(
            r#"
            SELECT CASE
                     WHEN jsonb_typeof(sheet.sheet_json -> 'skills' -> $3) = 'number'
                     THEN (sheet.sheet_json -> 'skills' ->> $3)::integer
                   END
              FROM characters AS character
              JOIN character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.version = character.current_sheet_version
             WHERE character.campaign_id = $1
               AND character.character_id = $2
               AND character.state = 'APPROVED'
               AND sheet.locked
            "#,
        )
        .bind(&campaign_id)
        .bind(&draft.character_id)
        .bind(&draft.skill_name)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_skill_target"))?
        .flatten();
        let target = target
            .and_then(|target| u8::try_from(target).ok())
            .filter(|target| *target <= 100)
            .ok_or(WorkflowStoreError::NotFound)?;
        let roll = generate_roll(target)?;
        validate_identifier(&roll.execution_id, "skill_check_execution_id_required")?;
        let reconstructed = if roll.selected_tens_digit == 0 && roll.ones_digit == 0 {
            100
        } else {
            roll.selected_tens_digit * 10 + roll.ones_digit
        };
        if !(1..=100).contains(&roll.roll)
            || roll.selected_tens_digit > 9
            || roll.ones_digit > 9
            || reconstructed != roll.roll
            || !matches!(
                roll.success_level.as_str(),
                "CRITICAL" | "EXTREME" | "HARD" | "REGULAR" | "FAILURE" | "FUMBLE"
            )
        {
            return Err(WorkflowStoreError::Validation(
                "invalid_agent_skill_check_roll",
            ));
        }
        let result_json = normalize_json(
            &serde_json::json!({
                "adjustment": "NONE",
                "character_id": draft.character_id,
                "ones_digit": roll.ones_digit,
                "random_source": "SERVER_OS_CSPRNG",
                "roll": roll.roll,
                "roll_id": roll.execution_id,
                "schema_version": 1,
                "selected_tens_digit": roll.selected_tens_digit,
                "skill_name": draft.skill_name,
                "success_level": roll.success_level,
                "target": target,
            })
            .to_string(),
        )?;
        let result_hash = labelled_sha256(result_json.as_bytes());
        sqlx::query(
            r#"
            INSERT INTO agent_job_tool_receipts (
                job_id, idempotency_key, tool_name, request_json,
                execution_id, result_json, result_hash
            ) VALUES (
                $1, $2, 'request_skill_check', $3::jsonb,
                $4, $5::jsonb, $6
            )
            "#,
        )
        .bind(&draft.job_id)
        .bind(&draft.idempotency_key)
        .bind(&request_json)
        .bind(&roll.execution_id)
        .bind(&result_json)
        .bind(&result_hash)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database("insert_agent_tool_receipt"))?;
        let receipt = DurableAgentJobToolReceipt {
            execution_id: roll.execution_id,
            result_json,
            result_hash,
        };
        transaction
            .commit()
            .await
            .map_err(|_| WorkflowStoreError::Database("commit_agent_tool_receipt"))?;
        Ok(receipt)
    }

    pub async fn load_agent_skill_target(
        &self,
        campaign_id: &str,
        character_id: &str,
        skill_name: &str,
    ) -> Result<u8, WorkflowStoreError> {
        validate_identifier(campaign_id, "campaign_id_required")?;
        validate_identifier(character_id, "character_id_required")?;
        if skill_name.trim().is_empty()
            || skill_name.len() > 128
            || skill_name.chars().any(char::is_control)
        {
            return Err(WorkflowStoreError::Validation("skill_name_invalid"));
        }
        let target: Option<i32> = sqlx::query_scalar(
            r#"
            SELECT CASE
                     WHEN jsonb_typeof(sheet.sheet_json -> 'skills' -> $3) = 'number'
                     THEN (sheet.sheet_json -> 'skills' ->> $3)::integer
                   END
              FROM characters AS character
              JOIN character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.version = character.current_sheet_version
             WHERE character.campaign_id = $1
               AND character.character_id = $2
               AND character.state = 'APPROVED'
               AND sheet.locked
            "#,
        )
        .bind(campaign_id)
        .bind(character_id)
        .bind(skill_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkflowStoreError::Database("load_agent_skill_target"))?
        .flatten();
        target
            .and_then(|target| u8::try_from(target).ok())
            .filter(|target| *target <= 100)
            .ok_or(WorkflowStoreError::NotFound)
    }
}
