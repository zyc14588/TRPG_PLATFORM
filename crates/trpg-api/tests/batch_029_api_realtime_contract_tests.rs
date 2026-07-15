mod common;

use std::collections::HashSet;

use trpg_api::contract_core::{
    append_api_contract_event, build_openapi_contract_document, http_api_adapter_contract,
    persistence_adapter_contract, realtime_adapter_contract, replay_visible_deltas,
    tool_permission_gate_contract, validate_domain_nats_subject, validate_nats_subject,
    validate_primary_adapter_boundaries, ApiRealtimeEventPayload, ProviderAccessPath,
    COMMAND_ENDPOINT, HTTP_FRAMEWORK, OPENAPI_GENERATOR, REALTIME_DELTA_SUBJECT,
    SQLX_EVENT_STORE_ADAPTER_BOUNDARY, WEBSOCKET_SYNC_ENDPOINT,
};
use trpg_api::{
    api, api_and_transport, api_realtime_contracts, nats_subject_contracts, provider,
    request_idempotency_contract, AuthorityMode, EventStore, FormalWritePath, PrincipalScope,
    TrpgError, Visibility,
};

#[test]
fn batch_029_maps_all_primary_contracts_to_current_safe_modules() {
    let contracts = api_realtime_contracts();
    assert_eq!(contracts.len(), 15);

    for prompt_id in [
        "CODEX-0066-07-API-REALTIME-CONTRACTS-831b0504c2",
        "CODEX-0067-07-API-REALTIME-CONTRACTS-1ccbeea1df",
        "CODEX-0068-07-API-REALTIME-CONTRACTS-2b78603401",
        "CODEX-0069-07-API-REALTIME-CONTRACTS-3cc61a7d01",
        "CODEX-0070-07-API-REALTIME-CONTRACTS-40bb6959f3",
        "CODEX-0071-07-API-REALTIME-CONTRACTS-3277264d0e",
        "CODEX-0072-07-API-REALTIME-CONTRACTS-513ac60dc8",
        "CODEX-0685-07-API-REALTIME-CONTRACTS-5d2e1fa760",
        "CODEX-0686-07-API-REALTIME-CONTRACTS-54d06d623d",
        "CODEX-0687-07-API-REALTIME-CONTRACTS-1d88035bc8",
        "CODEX-0688-07-API-REALTIME-CONTRACTS-991d938d5b",
        "CODEX-0689-07-API-REALTIME-CONTRACTS-4b17a0fb09",
        "CODEX-0695-07-API-REALTIME-CONTRACTS-f602cf5008",
        "CODEX-0696-07-API-REALTIME-CONTRACTS-8bf63a87bb",
        "CODEX-0698-07-API-REALTIME-CONTRACTS-a5b1a48fc3",
    ] {
        trpg_test_support::assert_normalized_prompt_id_exists(prompt_id);
    }

    let module_names: HashSet<_> = contracts
        .iter()
        .map(|contract| contract.module_name)
        .collect();
    assert_eq!(module_names.len(), contracts.len());
    assert!(module_names.contains("api_web_socket_g_rpc_schema"));
    assert!(module_names.contains("openapi_contract"));
    assert!(contracts
        .iter()
        .all(|contract| contract.uses_current_safe_names()));
}

#[test]
fn command_envelope_authority_and_direct_write_guards_are_enforced() {
    let api_contract = api::contract();
    let authority = common::human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();

    let mut missing_idempotency = common::command_for(&api_contract, 0, "");
    assert_eq!(
        append_api_contract_event(&mut store, &authority, &missing_idempotency, &api_contract)
            .unwrap_err(),
        TrpgError::MissingIdempotencyKey
    );
    assert!(store.events().is_empty());

    let wrong_authority =
        trpg_test_support::authority_contract("camp_b029", AuthorityMode::AiKp, 1).unwrap();
    missing_idempotency.idempotency_key = "idem_wrong_authority".to_owned();
    assert_eq!(
        append_api_contract_event(
            &mut store,
            &wrong_authority,
            &missing_idempotency,
            &api_contract
        )
        .unwrap_err(),
        TrpgError::AuthorityViolation
    );
    assert!(store.events().is_empty());

    let mut direct_agent = common::command_for(&api_contract, 0, "idem_direct_agent");
    direct_agent.write_path = FormalWritePath::DirectAgent;
    assert_eq!(
        append_api_contract_event(&mut store, &authority, &direct_agent, &api_contract)
            .unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );
    assert!(store.events().is_empty());
}

#[test]
fn expected_version_and_idempotency_are_enforced_by_event_store() {
    let api_contract = request_idempotency_contract::contract();
    let authority = common::human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();

    let conflict = common::command_for(&api_contract, 1, "idem_conflict");
    assert_eq!(
        append_api_contract_event(&mut store, &authority, &conflict, &api_contract).unwrap_err(),
        TrpgError::ExpectedVersionConflict {
            expected: 1,
            actual: 0,
        }
    );

    let mut command = common::command_for(&api_contract, 0, "idem_once");
    append_api_contract_event(&mut store, &authority, &command, &api_contract).unwrap();
    command.expected_version = 1;
    assert_eq!(
        append_api_contract_event(&mut store, &authority, &command, &api_contract).unwrap_err(),
        TrpgError::DuplicateCommand
    );
    assert_eq!(store.events().len(), 1);
}

#[test]
fn realtime_visibility_and_fact_provenance_survive_replay() {
    let api_contract = api_and_transport::contract();
    let authority = common::human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();
    let player_a = trpg_api::EntityId::new("player_a").unwrap();
    let player_b = trpg_api::EntityId::new("player_b").unwrap();
    let mut command = common::command_for(&api_contract, 0, "idem_private_delta");
    command.visibility = Visibility::private_to_player(player_a.clone());

    let event = append_api_contract_event(&mut store, &authority, &command, &api_contract).unwrap();
    assert_eq!(event.fact_provenance.reference.as_str(), "fact_001");
    assert_eq!(
        replay_visible_deltas(&store, &PrincipalScope::Player(player_a)).len(),
        1
    );
    assert!(replay_visible_deltas(&store, &PrincipalScope::Player(player_b)).is_empty());
    assert!(replay_visible_deltas(&store, &PrincipalScope::Public).is_empty());
}

#[test]
fn openapi_and_nats_contracts_expose_governed_metadata() {
    let contracts = api_realtime_contracts();
    let document = build_openapi_contract_document(&contracts);

    assert_eq!(document.command_endpoint, COMMAND_ENDPOINT);
    assert_eq!(document.framework, HTTP_FRAMEWORK);
    assert_eq!(document.generator, OPENAPI_GENERATOR);
    assert!(document.required_headers.contains(&"idempotency_key"));
    assert!(document.required_headers.contains(&"expected_version"));
    assert!(document.required_headers.contains(&"correlation_id"));
    assert!(document.schemas.contains(&"openapi_index.event_schema"));
    assert_eq!(document.websocket_delta_subject, REALTIME_DELTA_SUBJECT);
    assert!(document
        .policy_gates
        .contains(&"tool_permission_gate_default_deny"));
    assert!(document
        .adapter_boundaries
        .contains(&SQLX_EVENT_STORE_ADAPTER_BOUNDARY));

    nats_subject_contracts::validate_subject("trpg.api.command.dispatched").unwrap();
    nats_subject_contracts::validate_subject("trpg.realtime.delta.broadcast").unwrap();
    assert!(validate_nats_subject("trpg.api.*").is_err());
}

#[test]
fn provider_access_is_limited_to_agent_gateway_boundary() {
    let gateway = provider::evaluate_provider_access(ProviderAccessPath::AgentGateway).unwrap();
    assert!(gateway.allowed);
    assert!(gateway.audit_fields.contains(&"correlation_id"));
    assert_eq!(
        provider::evaluate_provider_access(ProviderAccessPath::AgentRuntimeAdapter).unwrap_err(),
        TrpgError::PolicyDenied
    );
    assert_eq!(
        provider::evaluate_provider_access(ProviderAccessPath::DirectModelProvider).unwrap_err(),
        TrpgError::PolicyDenied
    );
}

#[test]
fn primary_adapter_boundaries_cover_http_realtime_persistence_and_policy_gates() {
    validate_primary_adapter_boundaries().unwrap();

    let http = http_api_adapter_contract();
    assert_eq!(http.framework, "axum");
    assert_eq!(http.openapi_generator, "utoipa");
    assert_eq!(http.method, "POST");
    assert_eq!(http.route, "/campaigns/{id}/actions");
    assert!(http.required_headers.contains(&"idempotency_key"));
    assert!(http.policy_gates.contains(&"openfga_relationship_check"));
    assert!(http.policy_gates.contains(&"opa_context_policy"));
    assert!(http
        .policy_gates
        .contains(&"tool_permission_gate_default_deny"));

    let realtime = realtime_adapter_contract();
    assert_eq!(realtime.websocket_endpoint, WEBSOCKET_SYNC_ENDPOINT);
    assert!(realtime.visibility_filtered);
    assert!(realtime.reconnect_supported);
    assert!(realtime.multi_room_supported);
    assert_eq!(
        realtime.replayable_events,
        trpg_contracts::canonical_event_registry()
    );
    assert!(validate_domain_nats_subject("campaign.campaign_001.event.created").is_ok());
    assert!(validate_domain_nats_subject("campaign.*").is_err());

    let persistence = persistence_adapter_contract();
    assert_eq!(
        persistence.adapter_boundary,
        SQLX_EVENT_STORE_ADAPTER_BOUNDARY
    );
    assert_eq!(
        persistence.formal_state_write_boundary,
        "state_service_event_store_boundary"
    );
}

#[test]
fn tool_permission_gate_defaults_deny_and_preserves_agent_gateway_boundary() {
    let gate = tool_permission_gate_contract();
    assert!(!gate.default_allow);
    assert!(gate.formal_state_tools_require_agent_gateway);
    for required_check in [
        "authority_contract",
        "agent_permission_profile",
        "visibility",
        "fact_provenance",
        "tool_schema_version",
        "safety",
    ] {
        assert!(gate.checks.contains(&required_check));
    }
    assert!(gate.policy_gates.contains(&"openfga_relationship_check"));
    assert!(gate.policy_gates.contains(&"opa_context_policy"));
    assert!(gate
        .policy_gates
        .contains(&"tool_permission_gate_default_deny"));

    let gateway = provider::evaluate_provider_access(ProviderAccessPath::AgentGateway).unwrap();
    assert!(gateway.allowed);
    assert_eq!(
        provider::evaluate_provider_access(ProviderAccessPath::DirectModelProvider).unwrap_err(),
        TrpgError::PolicyDenied
    );
}
