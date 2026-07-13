use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use trpg_contracts::{
    CanonicalEvent, WireErrorCode, CANONICAL_EVENT_SCHEMA_ID, CANONICAL_EVENT_VERSION,
};

fn fixture(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let source = fs::read_to_string(path).expect("fixture must be readable");
    let fence = source.find("```json").expect("JSON fence must exist");
    let start = fence + source[fence..].find('\n').expect("fence newline") + 1;
    let end = start + source[start..].find("```").expect("closing JSON fence");
    source[start..end].to_owned()
}

fn ws(source: &str, mut at: usize) -> usize {
    while source
        .as_bytes()
        .get(at)
        .is_some_and(u8::is_ascii_whitespace)
    {
        at += 1;
    }
    at
}

// Returns the raw contents while advancing past a JSON string. Escaped quotes and
// unicode escapes are skipped as a unit so delimiters inside strings stay inert.
fn token<'a>(source: &'a str, at: &mut usize) -> &'a str {
    assert_eq!(source.as_bytes().get(*at), Some(&b'"'));
    *at += 1;
    let start = *at;
    loop {
        match source.as_bytes().get(*at).copied() {
            Some(b'"') => {
                let value = &source[start..*at];
                *at += 1;
                return value;
            }
            Some(b'\\') => {
                *at += 1;
                let escaped = source.as_bytes().get(*at).expect("complete escape");
                *at += if *escaped == b'u' { 5 } else { 1 };
                assert!(*at <= source.len(), "complete escape");
            }
            Some(0..=31) => panic!("control byte in JSON string"),
            Some(_) => *at += 1,
            None => panic!("unterminated JSON string"),
        }
    }
}

fn array<'a>(source: &'a str, key: &str) -> &'a str {
    let mut at = 0;
    while at < source.len() {
        if source.as_bytes()[at] != b'"' {
            at += 1;
            continue;
        }
        let candidate = token(source, &mut at);
        let colon = ws(source, at);
        if candidate != key || source.as_bytes().get(colon) != Some(&b':') {
            continue;
        }
        let start = ws(source, colon + 1);
        assert_eq!(source.as_bytes().get(start), Some(&b'['), "array field");
        at = start + 1;
        let mut depth = 1;
        while depth > 0 {
            match source.as_bytes()[at] {
                b'"' => drop(token(source, &mut at)),
                b'[' => {
                    depth += 1;
                    at += 1;
                }
                b']' => {
                    depth -= 1;
                    at += 1;
                }
                _ => at += 1,
            }
        }
        return &source[start + 1..at - 1];
    }
    panic!("missing array field {key}");
}

fn fields(source: &str, array_key: &str, field_key: &str, required: bool) -> Vec<String> {
    let source = array(source, array_key);
    let (mut at, mut depth, mut object_count) = (0, 0, 0);
    let mut values = Vec::new();
    while at < source.len() {
        match source.as_bytes()[at] {
            b'"' => {
                let key = token(source, &mut at);
                let colon = ws(source, at);
                if depth == 1 && key == field_key {
                    assert_eq!(source.as_bytes().get(colon), Some(&b':'), "field colon");
                    at = ws(source, colon + 1);
                    values.push(token(source, &mut at).to_owned());
                }
            }
            b'{' => {
                if depth == 0 {
                    object_count += 1;
                }
                depth += 1;
                at += 1;
            }
            b'}' => {
                assert!(depth > 0, "unexpected object close");
                depth -= 1;
                at += 1;
            }
            byte if depth == 0 && !byte.is_ascii_whitespace() && byte != b',' => {
                panic!("array items must be objects")
            }
            _ => at += 1,
        }
    }
    assert_eq!(depth, 0, "unterminated object");
    assert!(!required || values.len() == object_count, "missing field");
    values
}

#[test]
fn wire_error_registry_is_closed_unique_and_screaming_snake_case() {
    let mut unique = BTreeSet::new();
    for code in WireErrorCode::ALL {
        let value = code.as_str();
        assert!(
            !value.is_empty()
                && !value.starts_with('_')
                && !value.ends_with('_')
                && !value.contains("__")
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'),
            "invalid wire error code {value:?}"
        );
        assert!(unique.insert(value), "duplicate wire error code {value}");
        assert_eq!(WireErrorCode::lookup(value), Ok(*code));
    }
    assert_eq!(unique.len(), WireErrorCode::ALL.len());
    assert!(WireErrorCode::lookup("ToolPermissionDenied").is_err());
    assert_eq!(
        WireErrorCode::ToolPermissionDenied.as_str(),
        "TOOL_PERMISSION_DENIED"
    );
}

#[test]
fn every_non_core_production_error_is_registered() {
    const CODES: &str = "
        AUTHORITY_CONTRACT_IMMUTABLE INVALID_CONFIRMED_FACT_SOURCE MISSING_COMMAND_METADATA
        AGENT_TOOL_NOT_ALLOWED RESTORE_HASH_MISMATCH ROLLBACK_RUNBOOK_REQUIRED
        PROJECTION_REBUILD_HASH_MISMATCH EXTENSION_STATE_WRITE_FORBIDDEN
        EXTENSION_DIRECT_LLM_FORBIDDEN EXTENSION_DATABASE_WRITE_FORBIDDEN
        EXTENSION_TOOL_GATE_BYPASS_FORBIDDEN EXTENSION_AUTHORITY_CONTRACT_FORBIDDEN
        EXTENSION_DICE_FORGE_FORBIDDEN EXTENSION_VISIBILITY_LEAK_FORBIDDEN
        EXTENSION_CAPABILITY_DENIED EXTENSION_TOOL_GRANT_DENIED EXTENSION_OPENFGA_DENIED
        EXTENSION_OPA_DENIED EXTENSION_AUDIT_REQUIRED EXTENSION_COMPATIBILITY_FIELDS_MISSING
        EXTENSION_COMPATIBILITY_REJECTED";
    for code in CODES.split_ascii_whitespace() {
        assert_eq!(WireErrorCode::lookup(code).unwrap().as_str(), code);
    }
}

#[test]
fn canonical_event_registry_is_closed_versioned_and_schema_identified() {
    let mut names = BTreeSet::new();
    let mut schemas = BTreeSet::new();
    for event in CanonicalEvent::ALL {
        let descriptor = event.descriptor();
        assert_eq!(descriptor.event(), *event);
        assert_eq!(descriptor.name(), event.name());
        assert_eq!(descriptor.version(), CANONICAL_EVENT_VERSION);
        assert_eq!(descriptor.schema_id(), event.schema_id());
        assert_eq!(descriptor.schema_id(), CANONICAL_EVENT_SCHEMA_ID);
        assert!(names.insert(descriptor.name()), "duplicate event name");
        schemas.insert(descriptor.schema_id());
        assert_eq!(CanonicalEvent::lookup(descriptor.name()), Ok(*event));
    }
    assert_eq!(schemas, BTreeSet::from([CANONICAL_EVENT_SCHEMA_ID]));
    assert!(CanonicalEvent::lookup("ArbitraryEvent").is_err());
}

#[test]
fn fixture_event_union_exactly_matches_the_canonical_registry() {
    let golden = fixture("fixtures/event_store/golden_event_stream_expected.v1.json.md");
    let golden_names = fields(&golden, "events", "type", true);
    assert_eq!(
        golden_names,
        [
            "CampaignCreated",
            "AuthorityContractLocked",
            "CharacterSheetSubmitted",
            "CharacterSheetVersionLocked",
            "DiceRolled",
            "ClueRevealed",
        ]
    );
    let appended = fields(&golden, "append_tests", "append", false);
    assert_eq!(appended, ["SessionSummaryCreated", "SessionSummaryCreated"]);

    let shared = fixture("fixtures/stages/detailed/S01_foundation_shared_kernel.current.json.md");
    let shared_names = fields(&shared, "expected_events", "type", true);
    assert_eq!(shared_names, ["SharedKernelTypesValidated"]);
    let api = fixture("fixtures/stages/detailed/S08_api_ws_nats_expected.current.json.md");
    let api_names = fields(&api, "expected_events", "type", true);
    assert_eq!(
        api_names,
        [
            "ApiRequestAccepted",
            "WebSocketStateSynced",
            "NatsMessagePublished",
        ]
    );

    let fixture_names: BTreeSet<_> = golden_names
        .iter()
        .chain(&appended)
        .chain(&shared_names)
        .chain(&api_names)
        .map(String::as_str)
        .collect();
    let registry_names: BTreeSet<_> = CanonicalEvent::ALL
        .iter()
        .map(|event| event.name())
        .collect();
    assert_eq!(fixture_names, registry_names);
    for name in fixture_names {
        let event = CanonicalEvent::lookup(name).unwrap();
        assert_eq!(event.version(), CANONICAL_EVENT_VERSION);
        assert_eq!(event.schema_id(), CANONICAL_EVENT_SCHEMA_ID);
    }
}
