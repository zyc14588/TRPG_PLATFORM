use crate::{append_coc7_event, Coc7EventPayload};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatCondition {
    Able,
    MajorWound,
    Dying,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatTransition {
    pub before_hp: u8,
    pub after_hp: u8,
    pub damage: u8,
    pub condition: CombatCondition,
}

pub fn apply_damage(current_hp: u8, max_hp: u8, damage: u8) -> KernelResult<CombatTransition> {
    if max_hp == 0 || current_hp > max_hp {
        return Err(TrpgError::InvalidConfiguration("hit_point_range"));
    }

    let after_hp = current_hp.saturating_sub(damage);
    let major_wound_threshold = (max_hp / 2).max(1);
    let condition = if after_hp == 0 && damage >= max_hp {
        CombatCondition::Dead
    } else if after_hp == 0 {
        CombatCondition::Dying
    } else if damage >= major_wound_threshold {
        CombatCondition::MajorWound
    } else {
        CombatCondition::Able
    };

    Ok(CombatTransition {
        before_hp: current_hp,
        after_hp,
        damage,
        condition,
    })
}

pub fn record_combat_transition<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    transition: &CombatTransition,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.combat_transition_recorded",
        "combat_state_machine",
        format!(
            "hp {}->{} damage={} condition={:?}",
            transition.before_hp, transition.after_hp, transition.damage, transition.condition
        ),
    )
}
