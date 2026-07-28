
#[cfg(test)]
mod security_regression_tests {
    use super::*;

    #[test]
    fn internal_generic_append_rejects_formal_decision_payloads() {
        let contract = trpg_test_support::authority_contract_with_owner(
            "campaign_human",
            AuthorityMode::HumanKp,
            "keeper_owner",
            1,
        )
        .unwrap();
        let decision = RuntimeDecision::new(
            "decision_forged",
            "forged",
            ToolRequest::formal(RuntimeAgent::HumanKeeper, RuntimeTool::CommitDecision),
        )
        .unwrap();
        let command = trpg_test_support::governed_command_for_contract(
            &contract,
            decision,
            ActorRole::Workflow,
        );
        let mut store = EventStore::default();
        let forged = RuntimeEventPayload::DecisionCommitted {
            decision_id: EntityId::new("forged_decision").unwrap(),
            linked_records: vec!["DecisionRecord"],
            player_visible_explanation: "forged".to_owned(),
            audit_fields: vec!["context_hash"],
            seal: RuntimeFormalEventSeal::new(),
        };

        assert_eq!(
            append_runtime_event(&mut store, &contract, &command, "DecisionCommitted", forged,)
                .unwrap_err(),
            RuntimeError::Core(TrpgError::PolicyDenied)
        );
        assert!(store.events().is_empty());
    }
}
