
impl CombatState {
    pub fn start(
        combat_id: impl Into<String>,
        participants: Vec<CombatantState>,
    ) -> KernelResult<Self> {
        let combat_id = combat_id.into();
        let unique: HashSet<&str> = participants
            .iter()
            .map(|participant| participant.participant_id.as_str())
            .collect();
        if !valid_combat_id(&combat_id)
            || participants.len() < 2
            || unique.len() != participants.len()
        {
            return Err(TrpgError::InvalidConfiguration("combat_state"));
        }

        let mut initiative = participants
            .iter()
            .map(|participant| (participant.dexterity, participant.participant_id.clone()))
            .collect::<Vec<_>>();
        initiative.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        Ok(Self {
            combat_id,
            participants,
            initiative_order: initiative.into_iter().map(|(_, id)| id).collect(),
            round: 1,
            current_turn_index: 0,
            turn_action_consumed: false,
            consumed_roll_ids: Vec::new(),
            status: CombatStatus::Ongoing,
            version: 1,
            last_transition: CombatMutation::Started,
        })
    }

    pub fn combat_id(&self) -> &str {
        &self.combat_id
    }

    pub fn participants(&self) -> &[CombatantState] {
        &self.participants
    }

    pub fn initiative_order(&self) -> &[String] {
        &self.initiative_order
    }

    pub const fn round(&self) -> u32 {
        self.round
    }

    pub const fn current_turn_index(&self) -> usize {
        self.current_turn_index
    }

    pub const fn turn_action_consumed(&self) -> bool {
        self.turn_action_consumed
    }

    pub const fn status(&self) -> CombatStatus {
        self.status
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn current_actor(&self) -> Option<&str> {
        (self.status == CombatStatus::Ongoing)
            .then(|| self.initiative_order.get(self.current_turn_index))
            .flatten()
            .map(String::as_str)
    }

    pub fn apply_damage(
        &mut self,
        target_id: &str,
        action: CombatActionKind,
        defense: CombatDefense,
        attacker_roll: &ServerPercentileRoll,
        defender_roll: Option<&ServerPercentileRoll>,
        damage_roll: Option<&ServerDamageRoll>,
    ) -> KernelResult<CombatTransition> {
        let attacker_id = self
            .current_actor()
            .ok_or(TrpgError::InvalidConfiguration("combat_actor"))?
            .to_owned();
        let attacker = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == attacker_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_actor"))?;
        if !attacker.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration(
                "combat_actor_incapacitated",
            ));
        }
        if self.turn_action_consumed {
            return Err(TrpgError::InvalidConfiguration(
                "combat_turn_action_consumed",
            ));
        }
        let attacker_target = attacker.skill_targets.attack_target(action);
        let defender = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == target_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        if defense != CombatDefense::None && !defender.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration(
                "combat_defender_incapacitated",
            ));
        }
        if defender.condition == CombatCondition::Dead {
            return Err(TrpgError::InvalidConfiguration("combat_target_dead"));
        }
        if (defense != CombatDefense::None) != defender_roll.is_some() {
            return Err(TrpgError::InvalidConfiguration("combat_defense_roll"));
        }
        if defense == CombatDefense::FightBack && action != CombatActionKind::Melee {
            return Err(TrpgError::InvalidConfiguration("combat_fight_back_action"));
        }
        let defender_target = defender.skill_targets.defense_target(defense);
        if defense == CombatDefense::None {
            let attacker_roll =
                PercentileRollEvidence::from_server_roll(attacker_target, attacker_roll)?;
            let damage_roll = damage_roll.map(DamageRollEvidence::from_server_roll);
            return self.apply_verified_attack(VerifiedAttackInput {
                attacker_id: &attacker_id,
                target_id,
                action,
                defense,
                attacker_roll: &attacker_roll,
                defender_roll: None,
                damage_roll: damage_roll.as_ref(),
            });
        }
        let defender_target =
            defender_target.ok_or(TrpgError::InvalidConfiguration("combat_defense_roll"))?;
        let attacker_roll =
            PercentileRollEvidence::from_server_roll(attacker_target, attacker_roll)?;
        let defender_roll = defender_roll
            .map(|roll| PercentileRollEvidence::from_server_roll(defender_target, roll))
            .transpose()?;
        let damage_roll = damage_roll.map(DamageRollEvidence::from_server_roll);
        self.apply_verified_attack(VerifiedAttackInput {
            attacker_id: &attacker_id,
            target_id,
            action,
            defense,
            attacker_roll: &attacker_roll,
            defender_roll: defender_roll.as_ref(),
            damage_roll: damage_roll.as_ref(),
        })
    }

    fn apply_verified_attack(
        &mut self,
        input: VerifiedAttackInput<'_>,
    ) -> KernelResult<CombatTransition> {
        let VerifiedAttackInput {
            attacker_id,
            target_id,
            action,
            defense,
            attacker_roll,
            defender_roll,
            damage_roll,
        } = input;
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        if self.current_actor() != Some(attacker_id) || attacker_id == target_id {
            return Err(TrpgError::InvalidConfiguration("combat_actor"));
        }
        let attacker = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == attacker_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_actor"))?;
        if !attacker.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration(
                "combat_actor_incapacitated",
            ));
        }
        if self.turn_action_consumed {
            return Err(TrpgError::InvalidConfiguration(
                "combat_turn_action_consumed",
            ));
        }
        let attacker_target = attacker.skill_targets.attack_target(action);
        let defender = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == target_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        if defense != CombatDefense::None && !defender.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration(
                "combat_defender_incapacitated",
            ));
        }
        if defender.condition == CombatCondition::Dead {
            return Err(TrpgError::InvalidConfiguration("combat_target_dead"));
        }
        let defender_target = defender.skill_targets.defense_target(defense);
        if defense == CombatDefense::FightBack && action != CombatActionKind::Melee {
            return Err(TrpgError::InvalidConfiguration("combat_fight_back_action"));
        }
        attacker_roll.validate(attacker_target)?;
        if (defense != CombatDefense::None) != defender_roll.is_some() {
            return Err(TrpgError::InvalidConfiguration("combat_defense_roll"));
        }
        if let (Some(defender_roll), Some(defender_target)) = (defender_roll, defender_target) {
            defender_roll.validate(defender_target)?;
        }
        let mut attack_roll_ids = vec![attacker_roll.roll_id.as_str()];
        if let Some(defender_roll) = defender_roll {
            attack_roll_ids.push(defender_roll.roll_id.as_str());
        }
        if attack_roll_ids.iter().collect::<HashSet<_>>().len() != attack_roll_ids.len()
            || attack_roll_ids.iter().any(|roll_id| {
                self.consumed_roll_ids
                    .iter()
                    .any(|consumed| consumed == roll_id)
            })
        {
            return Err(TrpgError::InvalidConfiguration("combat_roll_reuse"));
        }
        let outcome = exchange_outcome(
            defense,
            attacker_roll.success_level,
            defender_roll.map(|roll| roll.success_level),
        )?;
        let Some(outcome) = outcome else {
            if damage_roll.is_some() {
                return Err(TrpgError::InvalidConfiguration(
                    "combat_damage_roll_unexpected",
                ));
            }
            let target = self
                .participants
                .iter()
                .find(|participant| participant.participant_id == target_id)
                .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
            let transition = CombatTransition {
                before_hp: target.current_hp,
                after_hp: target.current_hp,
                raw_damage: 0,
                armor_absorbed: 0,
                damage: 0,
                prior_condition: target.condition,
                condition: target.condition,
            };
            self.last_transition = CombatMutation::AttackMissed {
                attacker_id: attacker_id.to_owned(),
                target_id: target_id.to_owned(),
                action,
                defense,
                attacker_roll: attacker_roll.clone(),
                defender_roll: defender_roll.cloned(),
            };
            self.consumed_roll_ids
                .extend(attack_roll_ids.into_iter().map(str::to_owned));
            self.turn_action_consumed = true;
            self.version = self
                .version
                .checked_add(1)
                .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
            return Ok(transition);
        };
        let damage_roll =
            damage_roll.ok_or(TrpgError::InvalidConfiguration("combat_damage_evidence"))?;
        attack_roll_ids.push(damage_roll.roll_id.as_str());
        if attack_roll_ids.iter().collect::<HashSet<_>>().len() != attack_roll_ids.len()
            || self
                .consumed_roll_ids
                .iter()
                .any(|consumed| attack_roll_ids.contains(&consumed.as_str()))
        {
            return Err(TrpgError::InvalidConfiguration("combat_roll_reuse"));
        }
        let damaged_participant_id = match outcome {
            CombatExchangeOutcome::AttackerHit => target_id,
            CombatExchangeOutcome::DefenderFoughtBack => attacker_id,
        };
        let expected_damage_formula = match outcome {
            CombatExchangeOutcome::AttackerHit => attacker.weapon_loadout.damage_formula(action),
            CombatExchangeOutcome::DefenderFoughtBack => defender
                .weapon_loadout
                .damage_formula(CombatActionKind::Melee),
        };
        damage_roll.validate(expected_damage_formula)?;
        let target = self
            .participants
            .iter_mut()
            .find(|participant| participant.participant_id == damaged_participant_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        let transition = apply_damage_with_armor(
            target.current_hp,
            target.max_hp,
            damage_roll.raw_damage,
            target.armor,
            target.condition,
        )?;
        target.current_hp = transition.after_hp;
        target.condition = transition.condition;
        self.last_transition = CombatMutation::DamageApplied {
            attacker_id: attacker_id.to_owned(),
            target_id: target_id.to_owned(),
            action,
            defense,
            outcome,
            attacker_roll: attacker_roll.clone(),
            defender_roll: defender_roll.cloned(),
            damage_roll: damage_roll.clone(),
            raw_damage: damage_roll.raw_damage,
        };
        self.consumed_roll_ids
            .extend(attack_roll_ids.into_iter().map(str::to_owned));
        self.turn_action_consumed = true;
        self.version = self
            .version
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
        Ok(transition)
    }

    pub fn recover_major_wound(
        &mut self,
        healer_id: &str,
        target_id: &str,
        medical_skill: CombatMedicalSkill,
        medical_roll: &ServerPercentileRoll,
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
        let medical_target = healer.skill_targets.medical_target(medical_skill);
        let medical_roll = PercentileRollEvidence::from_server_roll(medical_target, medical_roll)?;
        self.apply_verified_recovery(healer_id, target_id, medical_skill, &medical_roll)
    }
}
