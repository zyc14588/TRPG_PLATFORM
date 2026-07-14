use crate::{append_coc7_event, Coc7EventPayload};
use trpg_contracts::EventType;
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuccessLevel {
    Critical,
    Extreme,
    Hard,
    Regular,
    Failure,
    Fumble,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiceAdjustment {
    None,
    Bonus,
    Penalty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiceRollOutcome {
    pub target: u8,
    pub roll: u8,
    pub success_level: SuccessLevel,
    pub selected_tens_digit: u8,
    pub ones_digit: u8,
    pub adjustment: DiceAdjustment,
}

pub fn percentile_from_digits(tens_digit: u8, ones_digit: u8) -> KernelResult<u8> {
    if tens_digit > 9 || ones_digit > 9 {
        return Err(TrpgError::InvalidConfiguration("percentile_digit"));
    }

    let value = tens_digit * 10 + ones_digit;
    Ok(if value == 0 { 100 } else { value })
}

pub fn adjusted_percentile_roll(
    base_tens_digit: u8,
    ones_digit: u8,
    extra_tens_digits: &[u8],
    adjustment: DiceAdjustment,
) -> KernelResult<(u8, u8)> {
    let mut candidates = vec![(
        base_tens_digit,
        percentile_from_digits(base_tens_digit, ones_digit)?,
    )];
    for tens_digit in extra_tens_digits {
        candidates.push((
            *tens_digit,
            percentile_from_digits(*tens_digit, ones_digit)?,
        ));
    }

    let selected = match adjustment {
        DiceAdjustment::None => candidates[0],
        DiceAdjustment::Bonus => *candidates
            .iter()
            .min_by_key(|(_, roll)| *roll)
            .expect("non-empty candidates"),
        DiceAdjustment::Penalty => *candidates
            .iter()
            .max_by_key(|(_, roll)| *roll)
            .expect("non-empty candidates"),
    };

    Ok(selected)
}

pub fn success_level(roll: u8, target: u8) -> KernelResult<SuccessLevel> {
    if !(1..=100).contains(&roll) || target > 100 {
        return Err(TrpgError::InvalidConfiguration("skill_check_range"));
    }

    if roll == 1 {
        return Ok(SuccessLevel::Critical);
    }

    if (target < 50 && roll >= 96) || (target >= 50 && roll == 100) {
        return Ok(SuccessLevel::Fumble);
    }

    if roll <= target / 5 {
        Ok(SuccessLevel::Extreme)
    } else if roll <= target / 2 {
        Ok(SuccessLevel::Hard)
    } else if roll <= target {
        Ok(SuccessLevel::Regular)
    } else {
        Ok(SuccessLevel::Failure)
    }
}

pub fn adjudicate_skill_check(
    target: u8,
    base_tens_digit: u8,
    ones_digit: u8,
    extra_tens_digits: &[u8],
    adjustment: DiceAdjustment,
) -> KernelResult<DiceRollOutcome> {
    let (selected_tens_digit, roll) =
        adjusted_percentile_roll(base_tens_digit, ones_digit, extra_tens_digits, adjustment)?;
    let success_level = success_level(roll, target)?;

    Ok(DiceRollOutcome {
        target,
        roll,
        success_level,
        selected_tens_digit,
        ones_digit,
        adjustment,
    })
}

pub fn record_dice_roll_contract<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    outcome: &DiceRollOutcome,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        EventType::DiceRolled.name(),
        "dice_roll_contract",
        format!(
            "roll={} target={} level={:?}",
            outcome.roll, outcome.target, outcome.success_level
        ),
    )
}
