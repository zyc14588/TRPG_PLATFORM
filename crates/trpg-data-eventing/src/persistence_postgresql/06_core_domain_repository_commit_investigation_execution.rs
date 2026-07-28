
impl CoreDomainRepository {

    pub async fn commit_investigation_execution(
        &self,
        metadata: &CoreCommandMetadata,
        execution: &InvestigationExecutionRecord,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let pending = match self
            .load_pending_player_action(&execution.campaign_id, &execution.action_id)
            .await
        {
            Ok(pending) => pending,
            Err(CoreDomainRepositoryError::NotFound(_)) => {
                return self
                    .load_resolved_player_action_receipt(
                        metadata,
                        &execution.campaign_id,
                        &execution.action_id,
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        let PlayerActionIntentRecord::Investigation {
            skill_name,
            clue_id,
            clue_importance,
            adjustment,
        } = &pending.intent
        else {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "investigation_action_kind",
            ));
        };
        let sheet: serde_json::Value = serde_json::from_str(&pending.character_sheet_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_sheet_json"))?;
        let authoritative_target = sheet
            .get("skills")
            .and_then(|skills| skills.get(skill_name))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or(CoreDomainRepositoryError::Integrity(
                "character_skill_missing",
            ))?;
        validate_server_dice_record(&execution.dice, adjustment)?;
        let succeeded = matches!(
            execution.dice.success_level.as_str(),
            "CRITICAL" | "EXTREME" | "HARD" | "REGULAR"
        );
        let expected_clue_outcome = if succeeded {
            "REVEALED"
        } else if clue_importance == "CORE" {
            "REVEALED_WITH_COST"
        } else {
            "NOT_FOUND"
        };
        let expected_cost =
            (expected_clue_outcome == "REVEALED_WITH_COST").then_some("time_or_complication");
        if metadata.expected_version != 1
            || metadata.requesting_actor_id != execution.confirmed_by
            || metadata.requesting_actor_id != metadata.authority_owner
            || metadata.requesting_actor_role != "human_keeper"
            || metadata.provenance_kind != "human_keeper_statement"
            || execution.character_id != pending.character_id
            || execution.skill_name != *skill_name
            || execution.dice.target_value != authoritative_target
            || execution.clue_id != *clue_id
            || execution.clue_importance != *clue_importance
            || execution.clue_outcome != expected_clue_outcome
            || execution.clue_cost.as_deref() != expected_cost
            || execution.revealed_to_party != (expected_clue_outcome != "NOT_FOUND")
            || execution.resolved_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "investigation_execution",
            ));
        }
        self.ensure_campaign_admin(&execution.campaign_id, &execution.confirmed_by)
            .await?;
        for value in [
            execution.decision_id.as_str(),
            execution.tool_execution_id.as_str(),
            execution.clue_record_id.as_str(),
        ] {
            EntityId::new(value)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("execution_id"))?;
        }

        let outcome = serde_json::json!({
            "kind": "INVESTIGATION",
            "skill_name": skill_name,
            "success_level": execution.dice.success_level,
            "clue_outcome": execution.clue_outcome,
            "clue_cost": execution.clue_cost,
        });
        let projection = serde_json::json!({
            "kind": "CONFIRM_INVESTIGATION",
            "action_id": execution.action_id,
            "campaign_id": execution.campaign_id,
            "character_id": execution.character_id,
            "decision_id": execution.decision_id,
            "tool_execution_id": execution.tool_execution_id,
            "confirmed_by": execution.confirmed_by,
            "resolved_at_unix_ms": execution.resolved_at_unix_ms,
            "outcome": outcome,
            "roll_id": execution.dice.roll_id,
            "target_value": execution.dice.target_value,
            "rolled_value": execution.dice.rolled_value,
            "success_level": execution.dice.success_level,
            "selected_tens_digit": execution.dice.selected_tens_digit,
            "ones_digit": execution.dice.ones_digit,
            "adjustment": execution.dice.adjustment,
            "clue_record_id": execution.clue_record_id,
            "clue_id": execution.clue_id,
            "clue_importance": execution.clue_importance,
            "clue_outcome": execution.clue_outcome,
            "clue_cost": execution.clue_cost,
            "revealed_to_party": execution.revealed_to_party,
            "visibility_label": metadata.visibility_label,
            "visibility_subject": metadata.visibility_subject,
            "provenance_kind": metadata.provenance_kind,
            "provenance_reference": metadata.provenance_reference,
            "provenance_recorded_by": metadata.provenance_recorded_by,
        });
        let projection_id = self.player_action_projection_id(&projection).await?;
        let synthetic =
            || projection_target("core_domain.player_action_projection", &projection_id);
        let events = vec![
            player_action_event(
                "DiceRolled",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "roll_id": execution.dice.roll_id,
                    "target": execution.dice.target_value,
                    "roll": execution.dice.rolled_value,
                    "success_level": execution.dice.success_level,
                    "adjustment": execution.dice.adjustment,
                    "random_source": "SERVER_OS_CSPRNG",
                }),
                vec![
                    projection_target("public.dice_rolls", &execution.dice.roll_id),
                    synthetic(),
                ],
            )?,
            player_action_event(
                "SkillCheckResolved",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "skill_name": execution.skill_name,
                    "success_level": execution.dice.success_level,
                }),
                vec![synthetic()],
            )?,
            player_action_event(
                "ClueRevealed",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "clue_id": execution.clue_id,
                    "importance": execution.clue_importance,
                    "outcome": execution.clue_outcome,
                    "cost": execution.clue_cost,
                }),
                vec![
                    projection_target("public.clues", &execution.clue_record_id),
                    synthetic(),
                ],
            )?,
            player_action_event(
                "DecisionCommitted",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "tool_execution_id": execution.tool_execution_id,
                    "confirmed_by": execution.confirmed_by,
                    "outcome": outcome,
                }),
                vec![
                    projection_target("public.decision_records", &execution.decision_id),
                    projection_target("public.player_actions", &execution.action_id),
                    synthetic(),
                ],
            )?,
        ];
        let draft = metadata.to_player_action_draft(
            &execution.campaign_id,
            &execution.action_id,
            events,
        )?;
        let persisted = self
            .canonical
            .commit_player_action_projection(&draft, &projection)
            .await?;
        self.verify_player_action_commit(metadata, &execution.action_id, "RESOLVED", &persisted)
            .await?;
        Ok(persisted)
    }
}
