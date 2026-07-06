use trpg_data_eventing::{
    append_data_event, persistence_migrations, DataEventOperation, DataEventPayload,
    DataEventWrite, EventStore, FactProvenance, PrincipalScope, ProvenanceKind, TrpgError,
    Visibility, VisibilityLabel,
};
use trpg_data_eventing::{ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId};

const S03_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S03_stage_acceptance_fixture.v1.json.md");
const S03_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md"
);
const EVENT_STREAM_CASES: &str = include_str!("../../../test-data/event_store_stream_cases.md");
const RAG_SNAPSHOT_CASES: &str = include_str!("../../../test-data/rag_snapshot_cases.md");
const API_WS_CONTRACT_CASES: &str = include_str!("../../../test-data/api_ws_contract_samples.md");
const DATA_EVENTING_MIGRATION: &str =
    include_str!("../../../migrations/20260705000100_create_data_eventing_event_store.up.sql");

#[test]
fn s03_fixtures_are_bound_to_event_store_contract_assertions() {
    assert_contains_all(
        S03_STAGE_FIXTURE,
        &[
            "\"stage\": \"S03\"",
            "\"p1_findings_allowed\": 0",
            "\"may_weaken_tests\": false",
        ],
    );
    assert_contains_all(
        S03_DETAILED_FIXTURE,
        &[
            "\"expected_version\": 2",
            "\"ProjectionRebuilt\"",
            "\"wrong_expected_version\"",
            "\"duplicate_idempotency_key\"",
            "\"projection_hash_stable\"",
        ],
    );
    assert_contains_all(
        EVENT_STREAM_CASES,
        &[
            "\"private_to_player:user_player_a\"",
            "\"expected_version\":5",
            "\"VERSION_CONFLICT\"",
            "\"SECOND_RETURNS_EXISTING\"",
        ],
    );
}

#[test]
fn event_store_contract_enforces_version_idempotency_and_visibility() {
    let contract = AuthorityContract::new("campaign_camp_ai_harbor", AuthorityMode::AiKp, 1)
        .expect("valid authority contract");
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let player_a = EntityId::new("user_player_a").unwrap();
    let player_b = EntityId::new("user_player_b").unwrap();

    let seed = [
        (
            "CampaignCreated",
            Visibility::new(VisibilityLabel::SystemOnly),
        ),
        (
            "AuthorityContractLocked",
            Visibility::new(VisibilityLabel::SystemOnly),
        ),
        (
            "CharacterSheetSubmitted",
            Visibility::private_to_player(player_a.clone()),
        ),
        ("DiceRolled", Visibility::new(VisibilityLabel::PartyVisible)),
        (
            "ClueRevealed",
            Visibility::new(VisibilityLabel::PartyVisible),
        ),
    ];

    for (index, (event_type, visibility)) in seed.iter().enumerate() {
        let command = governed_command(
            index as u64,
            format!("idem_seed_{index}"),
            visibility.clone(),
        );
        append_data_event(
            &mut store,
            &contract,
            &command,
            DataEventWrite::new(
                "event_store_contract",
                event_type,
                DataEventOperation::EventStoreAppend,
                &["event_store"],
            ),
        )
        .unwrap();
    }

    let wrong_version = governed_command(
        4,
        "idem_wrong_expected_version",
        Visibility::new(VisibilityLabel::PartyVisible),
    );
    let err = append_data_event(
        &mut store,
        &contract,
        &wrong_version,
        DataEventWrite::new(
            "event_store_contract",
            "SessionSummaryCreated",
            DataEventOperation::EventStoreAppend,
            &["event_store"],
        ),
    )
    .unwrap_err();
    assert_eq!(
        err,
        TrpgError::ExpectedVersionConflict {
            expected: 4,
            actual: 5,
        }
    );

    let duplicate = governed_command(
        5,
        "idem_seed_0",
        Visibility::new(VisibilityLabel::PartyVisible),
    );
    let err = append_data_event(
        &mut store,
        &contract,
        &duplicate,
        DataEventWrite::new(
            "event_store_contract",
            "SessionSummaryCreated",
            DataEventOperation::EventStoreAppend,
            &["event_store"],
        ),
    )
    .unwrap_err();
    assert_eq!(err.code(), "DUPLICATE_COMMAND");
    assert_eq!(store.events().len(), 5);

    let allowed = governed_command(
        5,
        "idem_session_summary",
        Visibility::new(VisibilityLabel::PartyVisible),
    );
    append_data_event(
        &mut store,
        &contract,
        &allowed,
        DataEventWrite::new(
            "event_store_contract",
            "SessionSummaryCreated",
            DataEventOperation::EventStoreAppend,
            &["event_store", "event_outbox"],
        ),
    )
    .unwrap();

    assert_eq!(store.events().len(), 6);
    assert_eq!(
        store
            .replay_visible(&PrincipalScope::Player(player_a))
            .len(),
        1
    );
    assert!(store
        .replay_visible(&PrincipalScope::Player(player_b))
        .is_empty());
    assert_eq!(store.replay_visible(&PrincipalScope::PartyMember).len(), 3);
    assert_eq!(store.replay_visible(&PrincipalScope::System).len(), 6);
}

#[test]
fn migration_entry_is_repeatable_sqlx_evidence() {
    assert_contains_all(
        DATA_EVENTING_MIGRATION,
        &[
            "CREATE TABLE IF NOT EXISTS event_store",
            "CREATE TABLE IF NOT EXISTS event_outbox",
            "CREATE TABLE IF NOT EXISTS projection_checkpoint",
            "idempotency_key TEXT NOT NULL UNIQUE",
            "expected_version BIGINT NOT NULL",
            "authority_contract_version BIGINT NOT NULL",
            "visibility_label TEXT NOT NULL",
            "fact_provenance_kind TEXT NOT NULL",
            "correlation_id TEXT NOT NULL",
            "causation_id TEXT NOT NULL",
            "event_id BIGINT NOT NULL",
            "stream_id TEXT NOT NULL",
            "version BIGINT NOT NULL",
            "REFERENCES event_store(sequence)",
        ],
    );
    assert!(!DATA_EVENTING_MIGRATION.contains("v4"));
    assert!(!DATA_EVENTING_MIGRATION.contains("v5"));

    let statements = persistence_migrations::migration_statements();
    assert_eq!(statements.len(), 3);
    for (name, sql) in statements {
        assert!(trpg_data_eventing::is_current_safe_name(name));
        assert!(sql.contains("CREATE TABLE"));
    }
}

#[test]
fn rag_and_realtime_fixtures_preserve_metadata_and_private_visibility() {
    assert_contains_all(
        RAG_SNAPSHOT_CASES,
        &[
            "\"source_type\"",
            "\"visibility\"",
            "\"version\"",
            "\"owner\"",
            "\"allowed_use\"",
            "\"expected_player_context\":\"REDACTED\"",
        ],
    );
    assert_contains_all(
        API_WS_CONTRACT_CASES,
        &[
            "\"private_to_player:user_player_a\"",
            "\"must_not_deliver_to\":\"user_player_b\"",
            "\"expected_error\":\"AuthorityContractImmutable\"",
        ],
    );
}

fn governed_command(
    expected_version: u64,
    idempotency_key: impl Into<String>,
    visibility: Visibility,
) -> CommandEnvelope<()> {
    let idempotency_key = idempotency_key.into();
    let mut command = CommandEnvelope::governed((), ActorRole::Workflow, AuthorityMode::AiKp);
    command.command_id = EntityId::new(format!("command_{idempotency_key}")).unwrap();
    command.idempotency_key = idempotency_key.clone();
    command.expected_version = expected_version;
    command.visibility = visibility;
    command.fact_provenance =
        FactProvenance::new(ProvenanceKind::SystemFixture, "fact_s03", "fixture_s03").unwrap();
    command.correlation_id = EntityId::new(format!("corr_{idempotency_key}")).unwrap();
    command.causation_id = EntityId::new(format!("cause_{idempotency_key}")).unwrap();
    command
}

fn assert_contains_all(haystack: &str, needles: &[&str]) {
    for needle in needles {
        assert!(haystack.contains(needle), "missing fixture token: {needle}");
    }
}
