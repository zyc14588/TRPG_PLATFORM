
impl SkillGrowthRecord {
    pub fn from_server_roll(
        growth_event_id: impl Into<String>,
        character_id: impl Into<String>,
        source_sheet_version_id: impl Into<String>,
        new_sheet_version_id: impl Into<String>,
        skill_name: impl Into<String>,
        skill_before: u8,
        server_roll: &ServerGrowthRollEvidence,
    ) -> Result<Self, ConclusionError> {
        let skill_name = skill_name.into();
        if skill_name.trim().is_empty() || skill_name.len() > 128 || skill_before > 99 {
            return Err(ConclusionError::InvalidGrowth);
        }
        let improvement_check_roll = server_roll.improvement_check().value();
        let qualifies = skill_before < 99
            && (improvement_check_roll > skill_before || improvement_check_roll >= 96);
        let increase_roll = server_roll.increase().map(|roll| roll.value());
        if qualifies != increase_roll.is_some() {
            return Err(ConclusionError::InvalidGrowth);
        }
        let skill_after = increase_roll
            .map(|increase| skill_before.saturating_add(increase).min(99))
            .unwrap_or(skill_before);
        let source_sheet_version_id = EntityId::new(source_sheet_version_id)
            .map_err(|_| ConclusionError::InvalidIdentifier)?;
        let new_sheet_version_id =
            EntityId::new(new_sheet_version_id).map_err(|_| ConclusionError::InvalidIdentifier)?;
        if source_sheet_version_id == new_sheet_version_id {
            return Err(ConclusionError::InvalidGrowth);
        }
        Ok(Self {
            growth_event_id: EntityId::new(growth_event_id)
                .map_err(|_| ConclusionError::InvalidIdentifier)?,
            character_id: EntityId::new(character_id)
                .map_err(|_| ConclusionError::InvalidIdentifier)?,
            source_sheet_version_id,
            new_sheet_version_id,
            skill_name: skill_name.trim().to_owned(),
            skill_before,
            improvement_check_roll,
            increase_roll,
            skill_after,
            server_roll_id: EntityId::new(server_roll.improvement_check().roll_id())
                .map_err(|_| ConclusionError::InvalidIdentifier)?,
            increase_roll_id: server_roll
                .increase()
                .map(|roll| EntityId::new(roll.roll_id()))
                .transpose()
                .map_err(|_| ConclusionError::InvalidIdentifier)?,
        })
    }

    pub fn growth_event_id(&self) -> &EntityId {
        &self.growth_event_id
    }

    pub fn character_id(&self) -> &EntityId {
        &self.character_id
    }

    pub fn source_sheet_version_id(&self) -> &EntityId {
        &self.source_sheet_version_id
    }

    pub fn new_sheet_version_id(&self) -> &EntityId {
        &self.new_sheet_version_id
    }

    pub fn skill_name(&self) -> &str {
        &self.skill_name
    }

    pub const fn skill_before(&self) -> u8 {
        self.skill_before
    }

    pub const fn improvement_check_roll(&self) -> u8 {
        self.improvement_check_roll
    }

    pub const fn increase_roll(&self) -> Option<u8> {
        self.increase_roll
    }

    pub const fn skill_after(&self) -> u8 {
        self.skill_after
    }

    pub fn server_roll_id(&self) -> &EntityId {
        &self.server_roll_id
    }

    pub fn increase_roll_id(&self) -> Option<&EntityId> {
        self.increase_roll_id.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignConclusion {
    pub campaign_id: EntityId,
    pub session_id: EntityId,
    pub state: ConclusionState,
    pub ending: Option<EndingRecord>,
    pub growth: Vec<SkillGrowthRecord>,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConclusionError {
    InvalidIdentifier,
    SessionNotEnded,
    InvalidEnding,
    InvalidGrowth,
    DuplicateGrowth,
    InvalidTransition {
        from: ConclusionState,
        operation: &'static str,
    },
}

impl std::fmt::Display for ConclusionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentifier => formatter.write_str("CONCLUSION_INVALID_IDENTIFIER"),
            Self::SessionNotEnded => formatter.write_str("CONCLUSION_SESSION_NOT_ENDED"),
            Self::InvalidEnding => formatter.write_str("CONCLUSION_ENDING_INVALID"),
            Self::InvalidGrowth => formatter.write_str("CONCLUSION_GROWTH_INVALID"),
            Self::DuplicateGrowth => formatter.write_str("CONCLUSION_GROWTH_DUPLICATE"),
            Self::InvalidTransition { from, operation } => {
                write!(
                    formatter,
                    "CONCLUSION_INVALID_TRANSITION:{from:?}:{operation}"
                )
            }
        }
    }
}

impl std::error::Error for ConclusionError {}

impl CampaignConclusion {
    pub fn begin(
        campaign_id: impl Into<String>,
        session_id: impl Into<String>,
        session_state: DurableSessionState,
    ) -> Result<Self, ConclusionError> {
        if session_state != DurableSessionState::Ended {
            return Err(ConclusionError::SessionNotEnded);
        }
        Ok(Self {
            campaign_id: EntityId::new(campaign_id)
                .map_err(|_| ConclusionError::InvalidIdentifier)?,
            session_id: EntityId::new(session_id)
                .map_err(|_| ConclusionError::InvalidIdentifier)?,
            state: ConclusionState::AwaitingEnding,
            ending: None,
            growth: Vec::new(),
            version: 0,
        })
    }

    pub fn record_ending(
        &mut self,
        ending_event_id: impl Into<String>,
        ending_id: impl Into<String>,
        summary: impl Into<String>,
        ended_at_unix_ms: u64,
    ) -> Result<(), ConclusionError> {
        if self.state != ConclusionState::AwaitingEnding {
            return Err(ConclusionError::InvalidTransition {
                from: self.state,
                operation: "RECORD_ENDING",
            });
        }
        let summary = summary.into();
        if summary.trim().is_empty() || summary.len() > 1_024 || ended_at_unix_ms == 0 {
            return Err(ConclusionError::InvalidEnding);
        }
        self.ending = Some(EndingRecord {
            ending_event_id: EntityId::new(ending_event_id)
                .map_err(|_| ConclusionError::InvalidIdentifier)?,
            ending_id: EntityId::new(ending_id).map_err(|_| ConclusionError::InvalidIdentifier)?,
            summary: summary.trim().to_owned(),
            ended_at_unix_ms,
        });
        self.state = ConclusionState::AwaitingGrowth;
        self.version = 1;
        Ok(())
    }

    pub fn settle_growth(&mut self, growth: Vec<SkillGrowthRecord>) -> Result<(), ConclusionError> {
        if self.state != ConclusionState::AwaitingGrowth {
            return Err(ConclusionError::InvalidTransition {
                from: self.state,
                operation: "SETTLE_GROWTH",
            });
        }
        let mut event_ids = std::collections::HashSet::new();
        let mut character_skills = std::collections::HashSet::new();
        for record in &growth {
            if !event_ids.insert(record.growth_event_id.as_str())
                || !character_skills
                    .insert((record.character_id.as_str(), record.skill_name.as_str()))
            {
                return Err(ConclusionError::DuplicateGrowth);
            }
        }
        self.growth = growth;
        self.state = ConclusionState::Completed;
        self.version = 2;
        Ok(())
    }
}
