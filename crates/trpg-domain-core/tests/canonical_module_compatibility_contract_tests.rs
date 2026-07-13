use std::any::TypeId;

use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::character_combat_san_chase as character;
use trpg_domain_core::character_combat_san_chase_impl as character_compat;
use trpg_domain_core::command_cqrs::CommandAcceptedPayload;
use trpg_domain_core::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore};
use trpg_domain_core::investigation_clue_npc_time as investigation;
use trpg_domain_core::investigation_clue_npc_time_impl as investigation_compat;
use trpg_domain_core::rule_runtime_coc7 as rules;
use trpg_domain_core::rule_runtime_coc7_impl as rules_compat;

type CharacterRecord = fn(
    &DomainAuthorityContract,
    &mut EventStore<CommandAcceptedPayload>,
    &CommandEnvelope<&'static str>,
    character::CharacterCombatSanChaseDecision,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>>;

type InvestigationRecord = fn(
    &DomainAuthorityContract,
    &mut EventStore<CommandAcceptedPayload>,
    &CommandEnvelope<&'static str>,
    investigation::InvestigationClueNpcTimeDecision,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>>;

type RuleRecord = fn(
    &DomainAuthorityContract,
    &mut EventStore<CommandAcceptedPayload>,
    &CommandEnvelope<&'static str>,
    rules::Coc7RuleRuntimeDecision,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>>;

#[test]
fn impl_modules_preserve_canonical_type_identity() {
    assert_eq!(
        TypeId::of::<character::CharacterCombatSanChaseTrack>(),
        TypeId::of::<character_compat::CharacterCombatSanChaseTrack>()
    );
    assert_eq!(
        TypeId::of::<character::CharacterCombatSanChaseDecision>(),
        TypeId::of::<character_compat::CharacterCombatSanChaseDecision>()
    );
    assert_eq!(
        TypeId::of::<investigation::InvestigationClueNpcTimeTrack>(),
        TypeId::of::<investigation_compat::InvestigationClueNpcTimeTrack>()
    );
    assert_eq!(
        TypeId::of::<investigation::InvestigationClueNpcTimeDecision>(),
        TypeId::of::<investigation_compat::InvestigationClueNpcTimeDecision>()
    );
    assert_eq!(
        TypeId::of::<rules::Coc7RuleRuntimeDecision>(),
        TypeId::of::<rules_compat::Coc7RuleRuntimeDecision>()
    );
}

#[test]
fn impl_modules_preserve_canonical_function_signatures() {
    let _: CharacterRecord = character::record_character_combat_san_chase_decision::<&'static str>;
    let _: CharacterRecord =
        character_compat::record_character_combat_san_chase_decision::<&'static str>;

    let _: InvestigationRecord =
        investigation::record_investigation_clue_npc_time_decision::<&'static str>;
    let _: InvestigationRecord =
        investigation_compat::record_investigation_clue_npc_time_decision::<&'static str>;

    let _: RuleRecord = rules::record_rule_runtime_coc7_decision::<&'static str>;
    let _: RuleRecord = rules_compat::record_rule_runtime_coc7_decision::<&'static str>;
}
