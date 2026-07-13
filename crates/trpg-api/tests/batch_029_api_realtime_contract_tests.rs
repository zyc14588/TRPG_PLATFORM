mod common;

use std::collections::HashSet;

use trpg_api::contract_core::{
    append_api_contract_event, build_openapi_contract_document, http_api_adapter_contract,
    persistence_adapter_contract, realtime_adapter_contract, rebuild_api_projection,
    replay_visible_deltas, tool_permission_gate_contract,
    validate_api_projection_realtime_event_type, validate_domain_nats_subject,
    validate_nats_subject, validate_primary_adapter_boundaries, visible_realtime_delta,
    ApiRealtimeEventPayload, ProviderAccessPath, COMMAND_ENDPOINT, HTTP_FRAMEWORK,
    OPENAPI_GENERATOR, REALTIME_DELTA_SUBJECT, REALTIME_REPLAYABLE_EVENTS, REQUIRED_EVENT_FIELDS,
    SQLX_EVENT_STORE_ADAPTER_BOUNDARY, WEBSOCKET_SYNC_ENDPOINT,
};
use trpg_api::{
    api, api_and_transport, api_realtime_contracts, nats_subject_contracts, provider,
    request_idempotency_contract, AuthorityContract, AuthorityMode, CanonicalEvent, EventStore,
    FormalWritePath, PrincipalScope, TrpgError, Visibility,
};
use trpg_contracts::{CANONICAL_EVENT_SCHEMA_ID, CANONICAL_EVENT_VERSION};

#[test]
fn batch_029_maps_all_primary_contracts_to_current_safe_modules() {
    let contracts = api_realtime_contracts();
    assert_eq!(contracts.len(), 15);

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
    assert!(REQUIRED_EVENT_FIELDS.contains(&"event_descriptor"));
    assert!(contracts
        .iter()
        .all(|contract| contract.required_event_fields.contains(&"event_descriptor")));
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

    let wrong_authority = AuthorityContract::new("camp_b029", AuthorityMode::AiKp, 1).unwrap();
    missing_idempotency.idempotency_key = "idem_wrong_authority".to_owned();
    assert_eq!(
        append_api_contract_event(
            &mut store,
            &wrong_authority,
            &missing_idempotency,
            &api_contract
        )
        .unwrap_err(),
        TrpgError::AuthorityContractMutation
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
        replay_visible_deltas(&store, &PrincipalScope::Player(player_a))
            .unwrap()
            .len(),
        1
    );
    assert!(
        replay_visible_deltas(&store, &PrincipalScope::Player(player_b))
            .unwrap()
            .is_empty()
    );
    assert!(replay_visible_deltas(&store, &PrincipalScope::Public)
        .unwrap()
        .is_empty());
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
    let expected_canonical_events: Vec<_> = CanonicalEvent::ALL
        .iter()
        .copied()
        .map(CanonicalEvent::descriptor)
        .collect();
    assert_eq!(document.canonical_events, expected_canonical_events);
    assert_eq!(
        document.schemas.len(),
        document
            .schemas
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
    );
    let canonical_schema_ids: HashSet<_> = document
        .canonical_events
        .iter()
        .map(|event| event.schema_id())
        .collect();
    assert_eq!(canonical_schema_ids.len(), 1);
    assert!(canonical_schema_ids.contains(CANONICAL_EVENT_SCHEMA_ID));
    assert_eq!(
        document
            .schemas
            .iter()
            .filter(|schema| **schema == CANONICAL_EVENT_SCHEMA_ID)
            .count(),
        1
    );
    for event in &document.canonical_events {
        assert_eq!(CanonicalEvent::lookup(event.name()), Ok(event.event()));
        assert_eq!(event.version(), CANONICAL_EVENT_VERSION);
        assert_eq!(event.version(), event.event().version());
        assert_eq!(event.schema_id(), CANONICAL_EVENT_SCHEMA_ID);
        assert_eq!(event.schema_id(), event.event().schema_id());
        assert!(document.schemas.contains(&event.schema_id()));
    }
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
    assert_eq!(realtime.replayable_event_types, REALTIME_REPLAYABLE_EVENTS);
    assert!(realtime
        .replayable_event_types
        .iter()
        .all(|event| CanonicalEvent::lookup(event.name()) == Ok(*event)));
    assert!(realtime.nats_subjects.contains(&REALTIME_DELTA_SUBJECT));
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
fn projection_and_realtime_accept_registry_and_internal_events_but_reject_unknown_events() {
    let api_contract = api_and_transport::contract();
    let authority = common::human_contract();
    let mut store: EventStore<ApiRealtimeEventPayload> = EventStore::default();
    let command = common::command_for(&api_contract, 0, "idem_registry_validation");
    let internal_event =
        append_api_contract_event(&mut store, &authority, &command, &api_contract).unwrap();

    assert!(
        visible_realtime_delta(&internal_event, &PrincipalScope::System)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        rebuild_api_projection(&[internal_event.clone()])
            .unwrap()
            .event_count,
        1
    );

    for canonical_event in REALTIME_REPLAYABLE_EVENTS {
        let idempotency_key = format!("idem_{}", canonical_event.name().to_ascii_lowercase());
        let canonical_command = common::command_for(&api_contract, 0, &idempotency_key);
        authority.validate_command(&canonical_command).unwrap();
        let mut canonical_store = EventStore::default();
        let event = canonical_store
            .append_canonical(
                &canonical_command,
                *canonical_event,
                internal_event.payload.clone(),
            )
            .unwrap();
        assert_eq!(event.event_descriptor, Some(canonical_event.descriptor()));
        assert!(visible_realtime_delta(&event, &PrincipalScope::System)
            .unwrap()
            .is_some());
        assert_eq!(rebuild_api_projection(&[event]).unwrap().event_count, 1);
    }

    let mut missing_descriptor = internal_event.clone();
    missing_descriptor.event_type = CanonicalEvent::ApiRequestAccepted.name();
    assert_eq!(
        visible_realtime_delta(&missing_descriptor, &PrincipalScope::System).unwrap_err(),
        TrpgError::InvalidConfiguration("api_projection_realtime_event_descriptor")
    );
    assert_eq!(
        rebuild_api_projection(&[missing_descriptor.clone()]).unwrap_err(),
        TrpgError::InvalidConfiguration("api_projection_realtime_event_descriptor")
    );

    missing_descriptor.event_descriptor = Some(CanonicalEvent::NatsMessagePublished.descriptor());
    assert_eq!(
        visible_realtime_delta(&missing_descriptor, &PrincipalScope::System).unwrap_err(),
        TrpgError::InvalidConfiguration("api_projection_realtime_event_descriptor")
    );

    let mut unexpected_descriptor = internal_event.clone();
    unexpected_descriptor.event_descriptor = Some(CanonicalEvent::ApiRequestAccepted.descriptor());
    assert_eq!(
        rebuild_api_projection(&[unexpected_descriptor]).unwrap_err(),
        TrpgError::InvalidConfiguration("api_projection_realtime_event_descriptor")
    );

    let unknown_event_type = "UnknownCanonicalFixtureEvent";
    assert_eq!(
        validate_api_projection_realtime_event_type(unknown_event_type).unwrap_err(),
        TrpgError::InvalidConfiguration("api_projection_realtime_event_type")
    );
    let mut unknown_event = internal_event;
    unknown_event.event_type = unknown_event_type;
    assert_eq!(
        visible_realtime_delta(&unknown_event, &PrincipalScope::System).unwrap_err(),
        TrpgError::InvalidConfiguration("api_projection_realtime_event_type")
    );
    assert_eq!(
        rebuild_api_projection(&[unknown_event]).unwrap_err(),
        TrpgError::InvalidConfiguration("api_projection_realtime_event_type")
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
