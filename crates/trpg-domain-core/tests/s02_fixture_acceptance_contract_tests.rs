use trpg_domain_core::authority_contract::{
    patch_locked_authority_contract, DomainAuthorityContract,
};
use trpg_domain_core::ddd::{
    AuthorityMode, EntityId, FactProvenance, FactSource, PrincipalScope, ProvenanceKind,
    Visibility, VisibilityLabel,
};
use trpg_domain_core::fork_canon_lineage::{
    fork_campaign, CampaignForkRequest, CanonStatus, CopyScope,
};
use trpg_domain_core::visibility_fact_provenance::{
    most_restrictive_label, promote_fact_to_confirmed, redaction_for, DerivedObject,
    RedactionOutcome,
};

const S02_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S02_stage_acceptance_fixture.v1.json.md");
const S02_EXPECTED_RECORDS: &str = include_str!(
    "../../../fixtures/stages/detailed/S02_authority_event_expected_records.current.json.md"
);
const AUTHORITY_CASES: &str =
    include_str!("../../../fixtures/authority/authority_contract_cases.v1.json.md");
const FORK_CASES: &str = include_str!("../../../fixtures/authority/fork_lineage_cases.v1.json.md");
const VISIBILITY_CASES: &str = include_str!("../../../test-data/visibility_leakage_cases.md");

#[test]
fn s02_stage_fixture_declares_required_reports_and_policy() {
    assert_contains_all(
        S02_STAGE_FIXTURE,
        &[
            "\"stage\": \"S02\"",
            "\"docs/reports/stages/S02_ACCEPTANCE_EVIDENCE.md\"",
            "\"docs/reports/stages/S02_TEST_RESULTS.md\"",
            "\"docs/reports/stages/S02_TRACEABILITY.md\"",
            "\"p0_findings_allowed\": 0",
            "\"p1_findings_allowed\": 0",
            "\"may_weaken_tests\": false",
        ],
    );
}

#[test]
fn s02_detailed_fixture_maps_errors_events_and_records_to_domain_assertions() {
    assert_contains_all(
        S02_EXPECTED_RECORDS,
        &[
            "\"type\": \"AuthorityMutationRejected\"",
            "\"error\": \"AUTHORITY_CONTRACT_IMMUTABLE\"",
            "\"type\": \"CampaignForked\"",
            "\"type\": \"FactPromoted\"",
            "\"record\": \"DecisionRecord\"",
            "\"decided_by\"",
            "\"source_context_hash\"",
            "\"error\": \"INVALID_CONFIRMED_FACT_SOURCE\"",
        ],
    );

    let contract = DomainAuthorityContract::new_locked(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let authority_error =
        patch_locked_authority_contract(&contract, AuthorityMode::HumanKp, "user_human_kp")
            .unwrap_err();
    assert_eq!(authority_error.code(), "AUTHORITY_CONTRACT_IMMUTABLE");

    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "event_001",
        "rules_001",
    )
    .unwrap();
    let fact_error = promote_fact_to_confirmed(
        "fact_agent_draft",
        FactSource::AgentDraft,
        Visibility::new(VisibilityLabel::KeeperOnly),
        provenance.clone(),
    )
    .unwrap_err();
    assert_eq!(fact_error.code(), "INVALID_CONFIRMED_FACT_SOURCE");

    let confirmed = promote_fact_to_confirmed(
        "fact_game_event",
        FactSource::GameEvent,
        Visibility::new(VisibilityLabel::Public),
        provenance,
    )
    .unwrap();
    assert_eq!(confirmed.source, FactSource::GameEvent);
}

#[test]
fn s02_authority_and_fork_fixtures_map_to_domain_fork_contract() {
    assert_contains_all(
        AUTHORITY_CASES,
        &[
            "\"case_id\": \"authority_locked_human_kp\"",
            "\"case_id\": \"authority_locked_ai_kp_no_human_override\"",
            "\"case_id\": \"fork_changes_authority_only_in_child\"",
            "\"error\": \"AuthorityContractImmutable\"",
            "\"error\": \"AuthorityViolation\"",
            "\"events_appended\": 0",
            "\"parent_unchanged\": true",
            "\"ai_internal_memory\"",
        ],
    );
    assert_contains_all(
        FORK_CASES,
        &[
            "\"source_campaign_id\": \"camp_ai_harbor\"",
            "\"fork_source_session_id\": \"session_002\"",
            "\"canon_status\": \"what-if\"",
            "\"child_contract_locked\": true",
            "\"parent_authority_mode\": \"AI_KP\"",
            "\"child_authority_mode\": \"HUMAN_KP\"",
        ],
    );

    let parent = DomainAuthorityContract::new_locked(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let request = CampaignForkRequest::new(
        "camp_ai_harbor",
        "session_002",
        "camp_human_harbor_whatif",
        AuthorityMode::HumanKp,
        "user_human_kp",
        "player_requested_human_kp_branch",
        "sha256_9e5d1b0c5d0838e2a81b72b3d3f361ba58ee03b06f6df5a0e93e1b93cf90b5ae",
    )
    .unwrap();

    let fork = fork_campaign(&parent, &request).unwrap();
    assert!(fork.parent_unchanged);
    assert_eq!(fork.canon_status, CanonStatus::WhatIf);
    assert_eq!(parent.authority_mode, AuthorityMode::AiKp);
    assert_eq!(
        fork.child_authority_contract.authority_mode,
        AuthorityMode::HumanKp
    );
    assert!(fork.child_authority_contract.locked);
    assert!(fork.copied_by_default.contains(&CopyScope::PublicEvents));
    assert!(fork
        .requires_explicit_permission
        .contains(&CopyScope::AiInternalMemory));
}

#[test]
fn s02_visibility_fixture_cases_map_to_redaction_assertions() {
    assert_contains_all(
        VISIBILITY_CASES,
        &[
            "\"case_id\":\"keeper_secret_not_in_player_export\"",
            "\"expected\":\"REDACTED\"",
            "\"case_id\":\"ai_internal_never_exported\"",
            "\"expected\":\"REDACTED_OR_AUDIT_ONLY\"",
            "\"expected_label\":\"keeper_only\"",
        ],
    );

    let player_a = EntityId::new("user_player_a").unwrap();
    let player_b = EntityId::new("user_player_b").unwrap();
    assert_eq!(
        redaction_for(
            &Visibility::new(VisibilityLabel::KeeperOnly),
            DerivedObject::PlayerExport,
            &PrincipalScope::Player(player_a)
        ),
        RedactionOutcome::Redacted
    );
    assert_eq!(
        redaction_for(
            &Visibility::private_to_player(EntityId::new("user_player_a").unwrap()),
            DerivedObject::SessionSummaryParty,
            &PrincipalScope::Player(player_b)
        ),
        RedactionOutcome::Redacted
    );
    assert_eq!(
        redaction_for(
            &Visibility::new(VisibilityLabel::AiInternal),
            DerivedObject::AnyPlayerOrKeeperExport,
            &PrincipalScope::Keeper
        ),
        RedactionOutcome::RedactedOrAuditOnly
    );
    assert_eq!(
        most_restrictive_label(&[VisibilityLabel::Public, VisibilityLabel::KeeperOnly]),
        Some(VisibilityLabel::KeeperOnly)
    );
}

fn assert_contains_all(haystack: &str, needles: &[&str]) {
    for needle in needles {
        assert!(haystack.contains(needle), "missing fixture token: {needle}");
    }
}
