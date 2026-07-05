use crate::{append_coc7_event, Coc7EventPayload};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterCombatSanChaseTrack {
    Character,
    Combat,
    Sanity,
    Chase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageBonus {
    MinusTwo,
    MinusOne,
    None,
    PlusD4,
    PlusD6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coc7Characteristics {
    pub strength: u8,
    pub dexterity: u8,
    pub power: u8,
    pub constitution: u8,
    pub size: u8,
    pub appearance: u8,
    pub intelligence: u8,
    pub education: u8,
    pub luck: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DerivedStats {
    pub hit_points: u8,
    pub magic_points: u8,
    pub sanity: u8,
    pub luck: u8,
    pub movement_rate: u8,
    pub damage_bonus: DamageBonus,
    pub build: i8,
}

pub fn derive_character_stats(characteristics: Coc7Characteristics) -> KernelResult<DerivedStats> {
    let values = [
        characteristics.strength,
        characteristics.dexterity,
        characteristics.power,
        characteristics.constitution,
        characteristics.size,
        characteristics.appearance,
        characteristics.intelligence,
        characteristics.education,
        characteristics.luck,
    ];
    if values.iter().any(|value| *value == 0 || *value > 100) {
        return Err(TrpgError::InvalidConfiguration("characteristic_range"));
    }

    let physical_total = characteristics.strength as u16 + characteristics.size as u16;
    let (damage_bonus, build) = match physical_total {
        2..=64 => (DamageBonus::MinusTwo, -2),
        65..=84 => (DamageBonus::MinusOne, -1),
        85..=124 => (DamageBonus::None, 0),
        125..=164 => (DamageBonus::PlusD4, 1),
        _ => (DamageBonus::PlusD6, 2),
    };
    let movement_rate = match (
        characteristics.strength > characteristics.size,
        characteristics.dexterity > characteristics.size,
    ) {
        (true, true) => 9,
        (false, false) => 7,
        _ => 8,
    };

    Ok(DerivedStats {
        hit_points: ((characteristics.constitution as u16 + characteristics.size as u16) / 10)
            as u8,
        magic_points: characteristics.power / 5,
        sanity: characteristics.power,
        luck: characteristics.luck,
        movement_rate,
        damage_bonus,
        build,
    })
}

pub fn record_character_combat_san_chase_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    track: CharacterCombatSanChaseTrack,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.character_combat_san_chase_recorded",
        "character_combat_san_chase",
        format!("track={:?}", track),
    )
}
