
impl CombatState {

    fn apply_verified_recovery(
        &mut self,
        healer_id: &str,
        target_id: &str,
        medical_skill: CombatMedicalSkill,
        medical_roll: &PercentileRollEvidence,
    ) -> KernelResult<CombatCondition> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        let healer = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == healer_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_healer"))?;
        if self.current_actor() != Some(healer_id) || !healer.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration("combat_healer"));
        }
        if self.turn_action_consumed {
            return Err(TrpgError::InvalidConfiguration(
                "combat_turn_action_consumed",
            ));
        }
        medical_roll.validate(healer.skill_targets.medical_target(medical_skill))?;
        if self
            .consumed_roll_ids
            .iter()
            .any(|consumed| consumed == &medical_roll.roll_id)
        {
            return Err(TrpgError::InvalidConfiguration("combat_roll_reuse"));
        }
        let target = self
            .participants
            .iter_mut()
            .find(|participant| participant.participant_id == target_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        let stabilizes_dying = target.current_hp == 0
            && target.condition == CombatCondition::Dying
            && medical_skill == CombatMedicalSkill::FirstAid;
        let treats_major_wound =
            target.current_hp > 0 && target.condition == CombatCondition::MajorWound;
        if !stabilizes_dying && !treats_major_wound {
            return Err(TrpgError::InvalidConfiguration("major_wound_recovery"));
        }
        let recovered = success_rank(medical_roll.success_level) > 0;
        if recovered {
            if stabilizes_dying {
                target.current_hp = 1;
                target.condition = CombatCondition::MajorWound;
            } else {
                target.condition = recover_major_wound(target.current_hp, target.condition, true)?;
            }
        }
        self.last_transition = CombatMutation::MajorWoundRecoveryAttempted {
            healer_id: healer_id.to_owned(),
            target_id: target_id.to_owned(),
            medical_skill,
            medical_roll: medical_roll.clone(),
            recovered,
        };
        self.consumed_roll_ids.push(medical_roll.roll_id.clone());
        self.turn_action_consumed = true;
        self.version = self
            .version
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
        Ok(target.condition)
    }

    pub fn advance_turn(&mut self) -> KernelResult<&str> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        let participant_count = self.initiative_order.len();
        let mut next_turn_index = self.current_turn_index;
        let mut next_round = self.round;
        for _ in 0..participant_count {
            next_turn_index += 1;
            if next_turn_index == participant_count {
                next_turn_index = 0;
                next_round = next_round
                    .checked_add(1)
                    .ok_or(TrpgError::InvalidConfiguration("combat_round"))?;
            }
            let candidate = &self.initiative_order[next_turn_index];
            if self
                .participants
                .iter()
                .find(|participant| participant.participant_id == *candidate)
                .is_some_and(|participant| participant.condition.can_act())
            {
                self.current_turn_index = next_turn_index;
                self.round = next_round;
                self.turn_action_consumed = false;
                self.last_transition = CombatMutation::TurnAdvanced;
                self.version = self
                    .version
                    .checked_add(1)
                    .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
                return Ok(candidate);
            }
        }
        Err(TrpgError::InvalidConfiguration(
            "combat_no_active_participant",
        ))
    }

    pub fn end(&mut self) -> KernelResult<()> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        self.status = CombatStatus::Ended;
        self.last_transition = CombatMutation::Ended;
        self.version = self
            .version
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
        Ok(())
    }

    /// Serializes a state-machine-produced snapshot for a formal event. The
    /// aggregate deliberately does not implement `Deserialize`, so callers
    /// cannot manufacture a recordable state from arbitrary JSON.
    pub fn persistence_json(&self) -> KernelResult<String> {
        serde_json::to_string(self)
            .map_err(|_| TrpgError::InvalidConfiguration("combat_state_serialization"))
    }

    /// Proves that this opaque aggregate is either a valid initial state or
    /// exactly one legal successor of the persisted state. Persistence
    /// adapters must call this before appending a formal event.
    pub fn validate_persistence_transition(
        &self,
        previous_state_json: Option<&str>,
    ) -> KernelResult<()> {
        let Some(previous_state_json) = previous_state_json else {
            if self.version == 1
                && self.round == 1
                && self.current_turn_index == 0
                && !self.turn_action_consumed
                && self.consumed_roll_ids.is_empty()
                && self.status == CombatStatus::Ongoing
                && matches!(self.last_transition, CombatMutation::Started)
            {
                return Ok(());
            }
            return Err(TrpgError::InvalidConfiguration("combat_initial_state"));
        };

        let wire: CombatStateWire = serde_json::from_str(previous_state_json)
            .map_err(|_| TrpgError::InvalidConfiguration("combat_persisted_state"))?;
        let mut expected = Self::try_from_wire(wire)?;
        match &self.last_transition {
            CombatMutation::Started => {
                return Err(TrpgError::InvalidConfiguration(
                    "combat_transition_restarted",
                ));
            }
            CombatMutation::AttackMissed {
                attacker_id,
                target_id,
                action,
                defense,
                attacker_roll,
                defender_roll,
            } => {
                expected.apply_verified_attack(VerifiedAttackInput {
                    attacker_id,
                    target_id,
                    action: *action,
                    defense: *defense,
                    attacker_roll,
                    defender_roll: defender_roll.as_ref(),
                    damage_roll: None,
                })?;
            }
            CombatMutation::DamageApplied {
                attacker_id,
                target_id,
                action,
                defense,
                outcome: _,
                attacker_roll,
                defender_roll,
                damage_roll,
                raw_damage,
            } => {
                if *raw_damage != damage_roll.raw_damage {
                    return Err(TrpgError::InvalidConfiguration("combat_damage_evidence"));
                }
                expected.apply_verified_attack(VerifiedAttackInput {
                    attacker_id,
                    target_id,
                    action: *action,
                    defense: *defense,
                    attacker_roll,
                    defender_roll: defender_roll.as_ref(),
                    damage_roll: Some(damage_roll),
                })?;
            }
            CombatMutation::MajorWoundRecoveryAttempted {
                healer_id,
                target_id,
                medical_skill,
                medical_roll,
                recovered: _,
            } => {
                expected.apply_verified_recovery(
                    healer_id,
                    target_id,
                    *medical_skill,
                    medical_roll,
                )?;
            }
            CombatMutation::TurnAdvanced => {
                expected.advance_turn()?;
            }
            CombatMutation::Ended => {
                expected.end()?;
            }
        }
        if expected == *self {
            Ok(())
        } else {
            Err(TrpgError::InvalidConfiguration(
                "combat_transition_mismatch",
            ))
        }
    }

    /// Validates an Event Store snapshot during projection replay without
    /// exposing a deserializable/constructible formal aggregate to callers.
    pub fn validate_serialized_persistence_transition(
        previous_state_json: Option<&str>,
        next_state_json: &str,
    ) -> KernelResult<()> {
        let wire: CombatStateWire = serde_json::from_str(next_state_json)
            .map_err(|_| TrpgError::InvalidConfiguration("combat_persisted_state"))?;
        let next = Self::try_from_wire(wire)?;
        next.validate_persistence_transition(previous_state_json)
    }

    fn try_from_wire(wire: CombatStateWire) -> KernelResult<Self> {
        let participants = wire
            .participants
            .into_iter()
            .map(|participant| CombatantState {
                participant_id: participant.participant_id,
                dexterity: participant.dexterity,
                skill_targets: participant.skill_targets,
                weapon_loadout: participant.weapon_loadout,
                current_hp: participant.current_hp,
                max_hp: participant.max_hp,
                armor: participant.armor,
                condition: participant.condition,
            })
            .collect::<Vec<_>>();
        let unique = participants
            .iter()
            .map(|participant| participant.participant_id.as_str())
            .collect::<HashSet<_>>();
        let mut derived_order = participants
            .iter()
            .map(|participant| (participant.dexterity, participant.participant_id.clone()))
            .collect::<Vec<_>>();
        derived_order
            .sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        let derived_order = derived_order
            .into_iter()
            .map(|(_, participant_id)| participant_id)
            .collect::<Vec<_>>();
        if !valid_combat_id(&wire.combat_id)
            || participants.len() < 2
            || unique.len() != participants.len()
            || participants.iter().any(|participant| {
                !valid_combat_id(&participant.participant_id)
                    || participant.dexterity == 0
                    || participant.dexterity > 100
                    || !(1..=100).contains(&participant.skill_targets.melee)
                    || !(1..=100).contains(&participant.skill_targets.firearm)
                    || !(1..=100).contains(&participant.skill_targets.dodge)
                    || !(1..=100).contains(&participant.skill_targets.first_aid)
                    || !(1..=100).contains(&participant.skill_targets.medicine)
                    || !participant.weapon_loadout.is_valid()
                    || participant.max_hp == 0
                    || participant.current_hp > participant.max_hp
                    || participant.armor > 30
                    || (participant.current_hp == 0)
                        != matches!(
                            participant.condition,
                            CombatCondition::Dying | CombatCondition::Dead
                        )
            })
            || derived_order != wire.initiative_order
            || wire.round == 0
            || wire.current_turn_index >= participants.len()
            || wire
                .consumed_roll_ids
                .iter()
                .any(|roll_id| !valid_combat_id(roll_id))
            || wire.consumed_roll_ids.iter().collect::<HashSet<_>>().len()
                != wire.consumed_roll_ids.len()
            || wire.version == 0
        {
            return Err(TrpgError::InvalidConfiguration("combat_persisted_state"));
        }
        Ok(Self {
            combat_id: wire.combat_id,
            participants,
            initiative_order: wire.initiative_order,
            round: wire.round,
            current_turn_index: wire.current_turn_index,
            turn_action_consumed: wire.turn_action_consumed,
            consumed_roll_ids: wire.consumed_roll_ids,
            status: wire.status,
            version: wire.version,
            last_transition: wire.last_transition,
        })
    }
}

pub fn resolve_attack(
    action: CombatActionKind,
    attacker_roll: &ServerDiceRoll,
    defense: CombatDefense,
    defender_roll: Option<&ServerDiceRoll>,
) -> KernelResult<AttackResolution> {
    if (defense != CombatDefense::None) != defender_roll.is_some() {
        return Err(TrpgError::InvalidConfiguration("combat_defense_roll"));
    }
    if defense == CombatDefense::FightBack && action != CombatActionKind::Melee {
        return Err(TrpgError::InvalidConfiguration("combat_fight_back_action"));
    }
    let attacker_success = attacker_roll.outcome().success_level;
    let defender_success = defender_roll.map(|roll| roll.outcome().success_level);
    let outcome = exchange_outcome(defense, attacker_success, defender_success)?;
    Ok(AttackResolution {
        action,
        defense,
        attacker_success,
        defender_success,
        hit: outcome == Some(CombatExchangeOutcome::AttackerHit),
        counterattack: outcome == Some(CombatExchangeOutcome::DefenderFoughtBack),
        outcome,
    })
}

pub fn apply_damage(
    current_hp: u8,
    max_hp: u8,
    damage: u8,
    prior_condition: CombatCondition,
) -> KernelResult<CombatTransition> {
    apply_damage_with_armor(current_hp, max_hp, damage, 0, prior_condition)
}
