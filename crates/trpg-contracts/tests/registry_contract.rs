use std::collections::HashSet;

use serde_json::Value;
use trpg_contracts::{
    canonical_event_registry, canonical_event_schema, canonical_openapi_components,
    validate_event_registry, CanonicalEventHeader, WireErrorCode,
};

const GOLDEN_STREAM: &str =
    include_str!("../../../fixtures/event_store/golden_event_stream_expected.v1.json.md");
const S05_EVENTS: &str = include_str!(
    "../../../fixtures/stages/detailed/S05_coc7_roll_san_combat_chase_expected.current.json.md"
);
const REGISTRY_FIXTURE: &str = include_str!("fixtures/canonical_event_registry.v1.json");

#[test]
fn event_registry_fixture_schema_openapi_and_projection_are_exactly_aligned() {
    validate_event_registry().unwrap();

    let golden = parse_markdown_json(GOLDEN_STREAM);
    let s05 = parse_markdown_json(S05_EVENTS);
    let mut fixture_events = HashSet::new();

    for event in golden["events"].as_array().unwrap() {
        fixture_events.insert(event_key(event, "type", "schema_version"));
    }
    for append in golden["append_tests"].as_array().unwrap() {
        if append.get("append").is_some() {
            fixture_events.insert(event_key(append, "append", "event_schema_version"));
        }
    }
    for event in s05["expected_events"].as_array().unwrap() {
        fixture_events.insert(event_key(event, "type", "schema_version"));
    }

    let registry_events = canonical_event_registry()
        .iter()
        .map(|descriptor| {
            (
                descriptor.name.to_owned(),
                u64::from(descriptor.schema_version),
            )
        })
        .collect::<HashSet<_>>();
    assert!(
        fixture_events.is_subset(&registry_events),
        "event stream fixture contains events absent from the canonical registry: {:?}",
        fixture_events
            .difference(&registry_events)
            .collect::<Vec<_>>()
    );

    let registry_fixture: Value = serde_json::from_str(REGISTRY_FIXTURE).unwrap();
    let fixture_contract = registry_fixture["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| {
            (
                event["name"].as_str().unwrap().to_owned(),
                event["schema_version"].as_u64().unwrap(),
                event["schema_id"].as_str().unwrap().to_owned(),
                event["projection"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<HashSet<_>>();
    let registry_contract = canonical_event_registry()
        .iter()
        .map(|descriptor| {
            (
                descriptor.name.to_owned(),
                u64::from(descriptor.schema_version),
                descriptor.schema_id.to_owned(),
                descriptor.projection.to_owned(),
            )
        })
        .collect::<HashSet<_>>();
    assert_eq!(fixture_contract, registry_contract);

    let schema = canonical_event_schema();
    let schema_variants = schema["oneOf"].as_array().unwrap();
    assert_eq!(schema_variants.len(), registry_events.len());
    for descriptor in canonical_event_registry() {
        assert!(schema_variants.iter().any(|entry| {
            entry["$id"] == descriptor.schema_id
                && entry["properties"]["type"]["const"] == descriptor.name
                && entry["properties"]["schema_version"]["const"] == descriptor.schema_version
        }));
        assert!(!descriptor.projection.is_empty());
    }

    let openapi = canonical_openapi_components();
    let openapi_events = openapi["EventEnvelope"]["events"].as_array().unwrap();
    assert_eq!(openapi_events.len(), registry_events.len());
    for descriptor in canonical_event_registry() {
        assert!(openapi_events.iter().any(|entry| {
            entry["name"] == descriptor.name
                && entry["schema_version"] == descriptor.schema_version
                && entry["schema_id"] == descriptor.schema_id
                && entry["projection"] == descriptor.projection
        }));
    }
}

#[test]
fn event_registry_rejects_unknown_names_and_version_drift() {
    let unknown = CanonicalEventHeader::parse("UnregisteredEvent", 1).unwrap_err();
    assert_eq!(unknown.code, WireErrorCode::EventContractUnknown);

    let mismatch = CanonicalEventHeader::parse("DiceRolled", 2).unwrap_err();
    assert_eq!(mismatch.code, WireErrorCode::EventContractVersionMismatch);
}

fn parse_markdown_json(markdown: &str) -> Value {
    let json = markdown
        .split_once("```json\n")
        .and_then(|(_, rest)| rest.split_once("\n```").map(|(body, _)| body))
        .expect("fixture must contain a fenced JSON object");
    serde_json::from_str(json).expect("fixture JSON must parse")
}

fn event_key(value: &Value, name_key: &str, version_key: &str) -> (String, u64) {
    (
        value[name_key].as_str().expect("event name").to_owned(),
        value[version_key].as_u64().expect("event schema version"),
    )
}
