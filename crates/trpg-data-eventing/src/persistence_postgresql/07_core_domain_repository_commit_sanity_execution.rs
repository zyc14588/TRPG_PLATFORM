
impl CoreDomainRepository {

    pub async fn commit_sanity_execution(
        &self,
        metadata: &CoreCommandMetadata,
        execution: &SanityExecutionRecord,
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
        let PlayerActionIntentRecord::SanityCheck {
            success_loss,
            failure_loss,
            day_key,
        } = &pending.intent
        else {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "sanity_action_kind",
            ));
        };
        let mut sheet: serde_json::Value = serde_json::from_str(&pending.character_sheet_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_sheet_json"))?;
        let power = sheet
            .pointer("/characteristics/power")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or(CoreDomainRepositoryError::Integrity(
                "character_power_missing",
            ))?;
        let existing_state = sheet.get("sanity_state");
        let same_day = existing_state
            .and_then(|state| state.get("day_key"))
            .and_then(serde_json::Value::as_str)
            == Some(day_key);
        let current_sanity = existing_state
            .and_then(|state| state.get("current_sanity"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(power);
        let day_start_sanity = if same_day {
            existing_state
                .and_then(|state| state.get("day_start_sanity"))
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "sanity_day_start_missing",
                ))?
        } else {
            current_sanity
        };
        let prior_day_loss = if same_day {
            existing_state
                .and_then(|state| state.get("day_loss"))
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "sanity_day_loss_missing",
                ))?
        } else {
            0
        };
        validate_server_dice_record(&execution.dice, "NONE")?;
        let succeeded = matches!(
            execution.dice.success_level.as_str(),
            "CRITICAL" | "EXTREME" | "HARD" | "REGULAR"
        );
        let expected_loss = if succeeded {
            *success_loss
        } else {
            *failure_loss
        };
        let expected_day_loss = prior_day_loss.saturating_add(expected_loss);
        let expected_threshold = (day_start_sanity / 5).max(1);
        let expected_after = current_sanity.saturating_sub(expected_loss);
        let expected_state = if expected_day_loss >= expected_threshold {
            "INDEFINITE_INSANITY"
        } else if expected_loss >= 5 {
            "TEMPORARY_INSANITY"
        } else {
            "STABLE"
        };
        if metadata.expected_version != 1
            || metadata.requesting_actor_id != execution.confirmed_by
            || metadata.requesting_actor_id != metadata.authority_owner
            || metadata.requesting_actor_role != "human_keeper"
            || metadata.provenance_kind != "human_keeper_statement"
            || execution.character_id != pending.character_id
            || execution.day_key != *day_key
            || execution.dice.target_value != current_sanity
            || execution.day_start_sanity != day_start_sanity
            || execution.sanity_before != current_sanity
            || execution.sanity_after != expected_after
            || execution.sanity_loss != expected_loss
            || execution.day_loss != expected_day_loss
            || execution.indefinite_threshold != expected_threshold
            || execution.madness_state != expected_state
            || execution.resolved_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput("sanity_execution"));
        }
        self.ensure_campaign_admin(&execution.campaign_id, &execution.confirmed_by)
            .await?;
        let sheet_version = pending.character_sheet_version.checked_add(1).ok_or(
            CoreDomainRepositoryError::Integrity("character_sheet_version_overflow"),
        )?;
        sheet["sanity_state"] = serde_json::json!({
            "day_key": day_key,
            "day_start_sanity": day_start_sanity,
            "current_sanity": expected_after,
            "day_loss": expected_day_loss,
            "madness_state": expected_state,
        });
        let outcome = serde_json::json!({
            "kind": "SANITY_CHECK",
            "success_level": execution.dice.success_level,
            "sanity_loss": expected_loss,
            "sanity_after": expected_after,
            "madness_state": expected_state,
        });
        let projection = serde_json::json!({
            "kind": "CONFIRM_SANITY",
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
            "sanity_event_id": execution.sanity_event_id,
            "sheet_version_id": execution.sheet_version_id,
            "sheet_version": sheet_version,
            "sheet_json": sheet,
            "day_key": execution.day_key,
            "day_start_sanity": execution.day_start_sanity,
            "sanity_before": execution.sanity_before,
            "sanity_after": execution.sanity_after,
            "sanity_loss": execution.sanity_loss,
            "day_loss": execution.day_loss,
            "indefinite_threshold": execution.indefinite_threshold,
            "madness_state": execution.madness_state,
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
                    "random_source": "SERVER_OS_CSPRNG",
                }),
                vec![
                    projection_target("public.dice_rolls", &execution.dice.roll_id),
                    synthetic(),
                ],
            )?,
            player_action_event(
                "SanityLossApplied",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "sanity_event_id": execution.sanity_event_id,
                    "day_key": execution.day_key,
                    "day_start_sanity": execution.day_start_sanity,
                    "sanity_before": execution.sanity_before,
                    "sanity_after": execution.sanity_after,
                    "loss": execution.sanity_loss,
                    "day_loss": execution.day_loss,
                    "indefinite_threshold": execution.indefinite_threshold,
                    "madness_state": execution.madness_state,
                }),
                vec![
                    projection_target("public.sanity_events", &execution.sanity_event_id),
                    projection_target(
                        "public.character_sheet_versions",
                        &execution.sheet_version_id,
                    ),
                    projection_target("public.characters", &execution.character_id),
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

fn projection_target(relation: &str, row_id: &str) -> CanonicalProjectionTarget {
    CanonicalProjectionTarget {
        relation: relation.to_owned(),
        row_id: row_id.to_owned(),
    }
}

fn campaign_fork_recorded_projection_targets(
    fork_id: &str,
    include_child_lineage_marker: bool,
) -> Vec<CanonicalProjectionTarget> {
    let mut targets = vec![projection_target("public.campaign_forks", fork_id)];
    if include_child_lineage_marker {
        // This legitimate command-owned row also acts as the HMAC-bound
        // discriminator for child-owned v2 lineage. The migration's partial
        // unique index can therefore exclude legacy parent-owned fork history.
        targets.push(projection_target(
            FORK_CHILD_LINEAGE_MARKER_RELATION,
            fork_id,
        ));
    }
    targets
}

fn gameplay_state_projection_targets(
    state_relation: &str,
    aggregate_id: &str,
    has_roll_consumptions: bool,
) -> Vec<CanonicalProjectionTarget> {
    let mut targets = vec![projection_target(state_relation, aggregate_id)];
    if has_roll_consumptions {
        targets.push(projection_target(
            "public.gameplay_roll_consumptions",
            aggregate_id,
        ));
    }
    targets
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GameplayRollConsumption {
    roll_id: String,
    roll_kind: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CombatHealthChange {
    character_id: String,
    hp_before: u8,
    hp_after: u8,
    condition_before: String,
    condition_after: String,
}

#[derive(Clone, Debug)]
struct PreparedCombatHealthProjection {
    update: CharacterCombatHealthUpdate,
    source_sheet_version_id: String,
    sheet_json: Value,
    visibility_label: String,
    visibility_subject: String,
}
