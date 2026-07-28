
fn validate_transition(draft: &WorkflowTransitionDraft) -> Result<(), WorkflowStoreError> {
    validate_identifier(&draft.workflow_id, "workflow_id_required")?;
    validate_identifier(&draft.idempotency_key, "idempotency_key_required")?;
    validate_identifier(&draft.correlation_id, "correlation_id_required")?;
    validate_identifier(&draft.causation_id, "causation_id_required")?;
    if draft.expected_version < 0 {
        return Err(WorkflowStoreError::Validation(
            "non_negative_expected_version_required",
        ));
    }
    validate_timestamp(draft.wake_at_unix_ms)?;
    if !draft.from_state.can_transition_to(draft.to_state) {
        return Err(WorkflowStoreError::StateConflict);
    }
    Ok(())
}

fn validate_identifier(value: &str, reason: &'static str) -> Result<(), WorkflowStoreError> {
    if value.trim().is_empty() || value.len() > 256 {
        Err(WorkflowStoreError::Validation(reason))
    } else {
        Ok(())
    }
}

fn validate_timestamp(value: Option<i64>) -> Result<(), WorkflowStoreError> {
    if value.is_some_and(|value| value < 0) {
        Err(WorkflowStoreError::Validation(
            "non_negative_wake_time_required",
        ))
    } else {
        Ok(())
    }
}

fn normalize_json(input: &str) -> Result<String, WorkflowStoreError> {
    if input.len() > 1_048_576 {
        return Err(WorkflowStoreError::Validation("workflow_json_too_large"));
    }
    let value: Value =
        serde_json::from_str(input).map_err(|_| WorkflowStoreError::Validation("invalid_json"))?;
    serde_json::to_string(&value).map_err(|_| WorkflowStoreError::Validation("invalid_json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_cannot_reenter_the_workflow() {
        assert!(!WorkflowState::Completed.can_transition_to(WorkflowState::Running));
        assert!(!WorkflowState::Failed.can_transition_to(WorkflowState::Running));
        assert!(!WorkflowState::Cancelled.can_transition_to(WorkflowState::Running));
    }

    #[test]
    fn remote_database_requires_hostname_verification() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let error = runtime
            .block_on(DurableWorkflowStore::connect(
                "postgresql://app@example.invalid/trpg?sslmode=require",
            ))
            .unwrap_err();
        assert_eq!(
            error,
            WorkflowStoreError::Configuration("remote_postgresql_requires_sslmode_verify_full")
        );
    }
}
