use crate::chase_state_machine::{advance_chase, ChaseObstacle, ChaseStatus};
use crate::combat_state_machine::{
    apply_damage_with_armor, resolve_attack, CombatActionKind, CombatCondition,
    CombatDamageFormula, CombatDefense,
};
use crate::dice_roll_contract::{
    server_roll_skill_check, DiceAdjustment, ServerDiceRoll, SuccessLevel,
};
use crate::{append_coc7_event, Coc7EventPayload};
use serde::Serialize;
use serde_json::Value;
use trpg_contracts::EventType;
use trpg_shared_kernel::{
    server_damage_roll, AuthorityContract, CommandEnvelope, EntityId, EventEnvelope, EventStore,
    KernelResult, ServerDamageRoll, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coc7EngineDecision {
    SkillCheck,
    OpposedRoll,
    SanityCheck,
    CombatRound,
    ChaseRound,
    InvestigationStep,
}

pub fn engine_decision_route(decision: Coc7EngineDecision) -> &'static str {
    match decision {
        Coc7EngineDecision::SkillCheck => "skill_check",
        Coc7EngineDecision::OpposedRoll => "opposed_roll",
        Coc7EngineDecision::SanityCheck => "sanity_check",
        Coc7EngineDecision::CombatRound => "combat_round",
        Coc7EngineDecision::ChaseRound => "chase_round",
        Coc7EngineDecision::InvestigationStep => "investigation_step",
    }
}

pub const fn canonical_event_for_decision(decision: Coc7EngineDecision) -> EventType {
    match decision {
        Coc7EngineDecision::SkillCheck | Coc7EngineDecision::OpposedRoll => {
            EventType::SkillCheckResolved
        }
        Coc7EngineDecision::SanityCheck => EventType::SanityLossApplied,
        Coc7EngineDecision::CombatRound => EventType::CombatStateUpdated,
        Coc7EngineDecision::ChaseRound => EventType::ChaseSegmentResolved,
        Coc7EngineDecision::InvestigationStep => EventType::ClueRevealed,
    }
}

pub fn record_coc7_rules_engine_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    decision: Coc7EngineDecision,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        canonical_event_for_decision(decision).name(),
        "coc7_rules_engine",
        format!("decision={}", engine_decision_route(decision)),
    )
}

/// Server-loaded, public-safe gameplay profiles. Transport adapters must
/// obtain these values from the approved character sheet and active scenario;
/// callers never supply rules statistics directly.
#[derive(Clone, Debug, PartialEq)]
pub struct PublicGameplayContext {
    pub npc_public_identity: String,
    pub character_combat_profile: Value,
    pub npc_combat_profile: Value,
    pub character_chase_profile: Value,
    pub npc_chase_profile: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicGameplayAction {
    NpcInteraction {
        character_id: String,
        npc_id: String,
        approach: String,
        public_response: String,
    },
    CombatRound {
        character_id: String,
        npc_id: String,
        action_kind: String,
        defense: String,
    },
    ChaseSegment {
        character_id: String,
        npc_id: String,
        initial_range: i8,
        obstacle_id: Option<String>,
        obstacle_cost: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicPercentileRoll {
    pub roll_id: String,
    pub target: u8,
    pub roll: u8,
    pub selected_tens_digit: u8,
    pub ones_digit: u8,
    pub success_level: String,
    pub adjustment: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicDamageRoll {
    pub roll_id: String,
    pub dice_count: u8,
    pub die_sides: u8,
    pub flat_bonus: i8,
    pub dice_values: Vec<u8>,
    pub value: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicGameplayResolution {
    NpcInteraction {
        summary: String,
        character_id: String,
        npc_id: String,
        npc_public_identity: String,
        approach: String,
        public_response: String,
    },
    CombatRound {
        summary: String,
        character_id: String,
        npc_id: String,
        action_kind: String,
        defense: String,
        hit: bool,
        counterattack: bool,
        before_hp: u8,
        after_hp: u8,
        damage: u8,
        armor_absorbed: u8,
        condition: String,
        attacker_roll: PublicPercentileRoll,
        defender_roll: Option<PublicPercentileRoll>,
        damage_roll: Option<PublicDamageRoll>,
        random_source: &'static str,
    },
    ChaseSegment {
        summary: String,
        character_id: String,
        npc_id: String,
        before_range: i8,
        after_range: i8,
        status: String,
        obstacle_id: Option<String>,
        obstacle_cost: u8,
        quarry_roll: PublicPercentileRoll,
        pursuer_roll: PublicPercentileRoll,
        random_source: &'static str,
    },
}

impl PublicGameplayResolution {
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::NpcInteraction { .. } => EventType::Coc7NpcDecisionRecorded.name(),
            Self::CombatRound { .. } => EventType::CombatStateUpdated.name(),
            Self::ChaseSegment { .. } => EventType::ChaseSegmentResolved.name(),
        }
    }
}

/// Resolves the RF04 public product slice entirely inside the COC7 rules
/// boundary. IDs and narrative choices are commands; all statistics come from
/// `PublicGameplayContext`, and every random value comes from the server OS
/// CSPRNG.
pub fn resolve_public_gameplay(
    context: &PublicGameplayContext,
    action: &PublicGameplayAction,
) -> KernelResult<PublicGameplayResolution> {
    match action {
        PublicGameplayAction::NpcInteraction {
            character_id,
            npc_id,
            approach,
            public_response,
        } => {
            validate_participant_ids(character_id, npc_id)?;
            if context.npc_public_identity.trim().is_empty()
                || context.npc_public_identity.len() > 256
                || approach.trim().is_empty()
                || approach.len() > 500
                || public_response.trim().is_empty()
                || public_response.len() > 2_000
            {
                return Err(TrpgError::InvalidConfiguration("npc_interaction"));
            }
            Ok(PublicGameplayResolution::NpcInteraction {
                summary: format!("与{}的互动已形成正式记录", context.npc_public_identity),
                character_id: character_id.clone(),
                npc_id: npc_id.clone(),
                npc_public_identity: context.npc_public_identity.clone(),
                approach: approach.clone(),
                public_response: public_response.clone(),
            })
        }
        PublicGameplayAction::CombatRound {
            character_id,
            npc_id,
            action_kind,
            defense,
        } => resolve_public_combat_round(context, character_id, npc_id, action_kind, defense),
        PublicGameplayAction::ChaseSegment {
            character_id,
            npc_id,
            initial_range,
            obstacle_id,
            obstacle_cost,
        } => resolve_public_chase_segment(
            context,
            character_id,
            npc_id,
            *initial_range,
            obstacle_id.as_deref(),
            *obstacle_cost,
        ),
    }
}

include!("coc7_rules_engine/01_public_combat_and_chase.rs");
