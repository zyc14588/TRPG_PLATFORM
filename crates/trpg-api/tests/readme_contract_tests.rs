mod common;

use trpg_api::contract_core::{
    build_openapi_contract_document, evaluate_provider_access, replay_visible_deltas,
    validate_api_contract, ApiRealtimeEventPayload, ProviderAccessPath,
};
use trpg_api::{
    readme, ActorRole, AuthorityContract, AuthorityMode, EntityId, EventStore, FormalWritePath,
    PrincipalScope, TrpgError, Visibility, VisibilityLabel,
};

#[test]
fn readme_contract_uses_current_safe_primary_metadata() {
    let contract = readme::contract();

    assert_eq!(contract.module_name, "readme");
    assert_eq!(contract.event_type, "ReadmeContractRecorded");
    assert_eq!(contract.event_schema_name, "readme.event_schema");
    assert!(contract.uses_current_safe_names());
    validate_api_contract(&contract).unwrap();

    for required_field in [
        "idempotency_key",
        "expected_version",
        "actor",
        "authority_mode",
        "visibility",
        "fact_provenance",
        "correlation_id",
        "causation_id",
        "write_path",
    ] {
        assert!(contract.required_command_fields.contains(&required_field));
    }

    for requirement in [
        "authority_contract_locked",
        "agent_gateway_only_ai_access",
        "tool_permission_gate_default_deny",
        "visibility_label_propagation",
        "fact_provenance_required",
        "event_store_is_canon",
        "formal_write_path_only",
        "no_private_realtime_leakage",
    ] {
        assert!(readme::readme_governance_requirements().contains(&requirement));
    }
}

#[test]
fn readme_rejects_authority_and_formal_write_boundary_violations() {
    let contract = readme::contract();
    let authority = common::human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();

    let missing_idempotency = common::command_for(&contract, 0, "");
    assert_eq!(
        readme::append_contract_event(&mut store, &authority, &missing_idempotency).unwrap_err(),
        TrpgError::MissingIdempotencyKey
    );

    let wrong_authority = AuthorityContract::new("camp_b029", AuthorityMode::AiKp, 1).unwrap();
    let authority_mismatch = common::command_for(&contract, 0, "idem_readme_wrong_authority");
    assert_eq!(
        readme::append_contract_event(&mut store, &wrong_authority, &authority_mismatch)
            .unwrap_err(),
        TrpgError::AuthorityContractMutation
    );

    let mut direct_agent = common::command_for(&contract, 0, "idem_readme_direct_agent");
    direct_agent.write_path = FormalWritePath::DirectAgent;
    assert_eq!(
        readme::append_contract_event(&mut store, &authority, &direct_agent).unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );

    let ai_direct_actor_payload = trpg_api::contract_core::ApiCommandPayload {
        module_name: contract.module_name,
        operation: contract.operation,
        request_summary: "readme direct ai actor must not commit formal state",
    };
    let mut ai_direct_actor = trpg_test_support::governed_command!(
        ai_direct_actor_payload,
        ActorRole::AiKeeper,
        AuthorityMode::AiKp,
    );
    ai_direct_actor.command_id = EntityId::new("command_readme_ai_direct").unwrap();
    ai_direct_actor.idempotency_key = "idem_readme_ai_direct".to_owned();
    let ai_authority = AuthorityContract::new("camp_b029", AuthorityMode::AiKp, 1).unwrap();
    assert_eq!(
        readme::append_contract_event(&mut store, &ai_authority, &ai_direct_actor).unwrap_err(),
        TrpgError::AuthorityViolation
    );

    assert!(store.events().is_empty());
}

#[test]
fn readme_commits_only_through_event_store_and_preserves_provenance() {
    let contract = readme::contract();
    let authority = common::human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();
    let mut command = common::command_for(&contract, 0, "idem_readme_once");
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);

    let event = readme::append_contract_event(&mut store, &authority, &command).unwrap();

    assert_eq!(store.events().len(), 1);
    assert_eq!(event.event_type, "ReadmeContractRecorded");
    assert_eq!(event.payload.module_name, "readme");
    assert_eq!(
        event.payload.endpoint,
        trpg_api::contract_core::COMMAND_ENDPOINT
    );
    assert_eq!(event.payload.event_schema_name, readme::EVENT_SCHEMA_NAME);
    assert_eq!(event.visibility.label(), &VisibilityLabel::KeeperOnly);
    assert_eq!(event.fact_provenance.reference.as_str(), "fact_001");
    assert_eq!(event.fact_provenance.recorded_by.as_str(), "rules_001");
    assert_eq!(event.correlation_id.as_str(), "corr_001");
    assert_eq!(event.causation_id.as_str(), "cause_001");

    assert!(replay_visible_deltas(&store, &PrincipalScope::Public)
        .unwrap()
        .is_empty());
    assert_eq!(
        replay_visible_deltas(&store, &PrincipalScope::Keeper)
            .unwrap()
            .len(),
        1
    );

    let mut duplicate = command;
    duplicate.expected_version = 1;
    assert_eq!(
        readme::append_contract_event(&mut store, &authority, &duplicate).unwrap_err(),
        TrpgError::DuplicateCommand
    );
    assert_eq!(store.events().len(), 1);
}

#[test]
fn readme_realtime_visibility_filters_private_and_ai_internal_content() {
    let contract = readme::contract();
    let authority = common::human_contract();
    let player_a = EntityId::new("player_a").unwrap();
    let player_b = EntityId::new("player_b").unwrap();

    let mut private_store: EventStore<ApiRealtimeEventPayload> = EventStore::default();
    let mut private_command = common::command_for(&contract, 0, "idem_readme_private");
    private_command.visibility = Visibility::private_to_player(player_a.clone());
    readme::append_contract_event(&mut private_store, &authority, &private_command).unwrap();

    assert_eq!(
        replay_visible_deltas(&private_store, &PrincipalScope::Player(player_a))
            .unwrap()
            .len(),
        1
    );
    assert!(
        replay_visible_deltas(&private_store, &PrincipalScope::Player(player_b))
            .unwrap()
            .is_empty()
    );
    assert!(
        replay_visible_deltas(&private_store, &PrincipalScope::Public)
            .unwrap()
            .is_empty()
    );

    let mut ai_store: EventStore<ApiRealtimeEventPayload> = EventStore::default();
    let mut ai_command = common::command_for(&contract, 0, "idem_readme_ai_internal");
    ai_command.visibility = Visibility::new(VisibilityLabel::AiInternal);
    readme::append_contract_event(&mut ai_store, &authority, &ai_command).unwrap();

    assert!(replay_visible_deltas(&ai_store, &PrincipalScope::Public)
        .unwrap()
        .is_empty());
    assert!(replay_visible_deltas(&ai_store, &PrincipalScope::Keeper)
        .unwrap()
        .is_empty());
    assert_eq!(
        replay_visible_deltas(&ai_store, &PrincipalScope::System)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn readme_exposes_openapi_nats_metrics_and_agent_gateway_boundary() {
    let contract = readme::contract();
    let document = build_openapi_contract_document(&[contract]);

    assert!(document.schemas.contains(&readme::EVENT_SCHEMA_NAME));
    assert!(document.required_headers.contains(&"idempotency_key"));
    assert!(document.required_headers.contains(&"expected_version"));
    assert!(document.required_headers.contains(&"correlation_id"));
    assert!(document
        .policy_gates
        .contains(&"openfga_relationship_check"));
    assert!(document.policy_gates.contains(&"opa_context_policy"));
    assert!(document
        .policy_gates
        .contains(&"tool_permission_gate_default_deny"));

    for subject in contract.nats_subjects {
        assert!(subject.starts_with("trpg."));
        assert!(!subject.contains('*'));
        assert!(!subject.contains('>'));
    }

    for metric in [
        "trpg_command_total",
        "trpg_event_append_latency_ms",
        "trpg_policy_deny_total",
        "trpg_projection_lag_events",
        "trpg_visibility_redaction_total",
    ] {
        assert!(contract.metrics.contains(&metric));
    }

    assert!(
        evaluate_provider_access(ProviderAccessPath::AgentGateway)
            .unwrap()
            .allowed
    );
    assert_eq!(
        evaluate_provider_access(ProviderAccessPath::DirectModelProvider).unwrap_err(),
        TrpgError::PolicyDenied
    );
    assert_eq!(
        evaluate_provider_access(ProviderAccessPath::AgentRuntimeAdapter).unwrap_err(),
        TrpgError::PolicyDenied
    );
}
