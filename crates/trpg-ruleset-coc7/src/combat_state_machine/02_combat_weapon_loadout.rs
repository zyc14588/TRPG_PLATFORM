
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatWeaponLoadout {
    melee: CombatWeapon,
    firearm: CombatWeapon,
}

impl CombatWeaponLoadout {
    pub fn new(melee: CombatWeapon, firearm: CombatWeapon) -> KernelResult<Self> {
        if !melee.is_valid() || !firearm.is_valid() {
            return Err(TrpgError::InvalidConfiguration("combat_weapon_loadout"));
        }
        Ok(Self { melee, firearm })
    }

    pub fn melee(&self) -> &CombatWeapon {
        &self.melee
    }

    pub fn firearm(&self) -> &CombatWeapon {
        &self.firearm
    }

    const fn damage_formula(&self, action: CombatActionKind) -> CombatDamageFormula {
        match action {
            CombatActionKind::Melee => self.melee.damage_formula,
            CombatActionKind::Firearm => self.firearm.damage_formula,
        }
    }

    fn is_valid(&self) -> bool {
        self.melee.is_valid() && self.firearm.is_valid()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CombatantState {
    participant_id: String,
    dexterity: u8,
    skill_targets: CombatSkillTargets,
    weapon_loadout: CombatWeaponLoadout,
    current_hp: u8,
    max_hp: u8,
    armor: u8,
    condition: CombatCondition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatHealth {
    current_hp: u8,
    max_hp: u8,
    condition: CombatCondition,
}

impl CombatHealth {
    pub fn new(current_hp: u8, max_hp: u8, condition: CombatCondition) -> KernelResult<Self> {
        if max_hp == 0
            || current_hp > max_hp
            || (current_hp == 0)
                != matches!(condition, CombatCondition::Dying | CombatCondition::Dead)
        {
            return Err(TrpgError::InvalidConfiguration("combat_health"));
        }
        Ok(Self {
            current_hp,
            max_hp,
            condition,
        })
    }

    pub const fn current_hp(self) -> u8 {
        self.current_hp
    }

    pub const fn max_hp(self) -> u8 {
        self.max_hp
    }

    pub const fn condition(self) -> CombatCondition {
        self.condition
    }
}

impl CombatantState {
    /// Constructs an encounter participant from the campaign's persisted
    /// health snapshot; starting a new combat never implies healing.
    pub fn new(
        participant_id: impl Into<String>,
        dexterity: u8,
        health: CombatHealth,
        armor: u8,
        skill_targets: CombatSkillTargets,
        weapon_loadout: CombatWeaponLoadout,
    ) -> KernelResult<Self> {
        let participant_id = participant_id.into();
        if !valid_combat_id(&participant_id)
            || dexterity == 0
            || dexterity > 100
            || armor > 30
            || !weapon_loadout.is_valid()
        {
            return Err(TrpgError::InvalidConfiguration("combat_participant"));
        }
        Ok(Self {
            participant_id,
            dexterity,
            skill_targets,
            weapon_loadout,
            current_hp: health.current_hp,
            max_hp: health.max_hp,
            armor,
            condition: health.condition,
        })
    }

    pub fn participant_id(&self) -> &str {
        &self.participant_id
    }

    pub const fn dexterity(&self) -> u8 {
        self.dexterity
    }

    pub const fn skill_targets(&self) -> CombatSkillTargets {
        self.skill_targets
    }

    pub const fn weapon_loadout(&self) -> &CombatWeaponLoadout {
        &self.weapon_loadout
    }

    pub const fn current_hp(&self) -> u8 {
        self.current_hp
    }

    pub const fn max_hp(&self) -> u8 {
        self.max_hp
    }

    pub const fn armor(&self) -> u8 {
        self.armor
    }

    pub const fn condition(&self) -> CombatCondition {
        self.condition
    }

    pub const fn health(&self) -> CombatHealth {
        CombatHealth {
            current_hp: self.current_hp,
            max_hp: self.max_hp,
            condition: self.condition,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
enum CombatMutation {
    Started,
    AttackMissed {
        attacker_id: String,
        target_id: String,
        action: CombatActionKind,
        defense: CombatDefense,
        attacker_roll: PercentileRollEvidence,
        defender_roll: Option<PercentileRollEvidence>,
    },
    DamageApplied {
        attacker_id: String,
        target_id: String,
        action: CombatActionKind,
        defense: CombatDefense,
        outcome: CombatExchangeOutcome,
        attacker_roll: PercentileRollEvidence,
        defender_roll: Option<PercentileRollEvidence>,
        damage_roll: DamageRollEvidence,
        raw_damage: u8,
    },
    MajorWoundRecoveryAttempted {
        healer_id: String,
        target_id: String,
        medical_skill: CombatMedicalSkill,
        medical_roll: PercentileRollEvidence,
        recovered: bool,
    },
    TurnAdvanced,
    Ended,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CombatState {
    combat_id: String,
    participants: Vec<CombatantState>,
    initiative_order: Vec<String>,
    round: u32,
    current_turn_index: usize,
    turn_action_consumed: bool,
    consumed_roll_ids: Vec<String>,
    status: CombatStatus,
    version: u64,
    last_transition: CombatMutation,
}

#[derive(Deserialize)]
struct CombatantStateWire {
    participant_id: String,
    dexterity: u8,
    skill_targets: CombatSkillTargets,
    weapon_loadout: CombatWeaponLoadout,
    current_hp: u8,
    max_hp: u8,
    armor: u8,
    condition: CombatCondition,
}

#[derive(Deserialize)]
struct CombatStateWire {
    combat_id: String,
    participants: Vec<CombatantStateWire>,
    initiative_order: Vec<String>,
    round: u32,
    current_turn_index: usize,
    turn_action_consumed: bool,
    consumed_roll_ids: Vec<String>,
    status: CombatStatus,
    version: u64,
    last_transition: CombatMutation,
}
