
impl CampaignInvite {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        invite_id: impl Into<String>,
        campaign_id: impl Into<String>,
        invited_user_id: impl Into<String>,
        issued_by: impl Into<String>,
        role: MembershipRole,
        token_digest: impl Into<String>,
        expires_at_unix_ms: u64,
        now_unix_ms: u64,
    ) -> CoreEntityResult<Self> {
        let token_digest = token_digest.into();
        if expires_at_unix_ms <= now_unix_ms {
            return Err(CoreEntityError::InviteExpired);
        }
        if !token_digest.strip_prefix("sha256:").is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(CoreEntityError::InvalidText("invite.token_digest"));
        }
        Ok(Self {
            invite_id: InviteId::new(invite_id)?,
            campaign_id: CampaignId::new(campaign_id)?,
            invited_user_id: UserId::new(invited_user_id)?,
            issued_by: UserId::new(issued_by)?,
            role,
            token_digest,
            expires_at_unix_ms,
        })
    }

    pub fn validate_acceptance(
        &self,
        accepting_user_id: &UserId,
        now_unix_ms: u64,
    ) -> CoreEntityResult<()> {
        if now_unix_ms >= self.expires_at_unix_ms {
            return Err(CoreEntityError::InviteExpired);
        }
        if accepting_user_id != &self.invited_user_id {
            return Err(CoreEntityError::InviteSubjectMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CharacterState {
    Draft,
    Submitted,
    Approved,
}

impl CharacterState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Submitted => "SUBMITTED",
            Self::Approved => "APPROVED",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub character_id: CharacterId,
    pub campaign_id: CampaignId,
    pub owner_user_id: UserId,
    pub display_name: String,
    pub state: CharacterState,
    pub current_sheet_version: u64,
    pub initial_version_locked: bool,
    pub version: u64,
}

impl Character {
    pub fn draft(
        character_id: impl Into<String>,
        campaign_id: impl Into<String>,
        owner_user_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> CoreEntityResult<Self> {
        Ok(Self {
            character_id: CharacterId::new(character_id)?,
            campaign_id: CampaignId::new(campaign_id)?,
            owner_user_id: UserId::new(owner_user_id)?,
            display_name: required_text(display_name, "character.display_name")?,
            state: CharacterState::Draft,
            current_sheet_version: 1,
            initial_version_locked: false,
            version: 1,
        })
    }

    pub fn submit(&mut self) -> CoreEntityResult<()> {
        if self.state != CharacterState::Draft || self.initial_version_locked {
            return Err(CoreEntityError::CharacterSheetAlreadyLocked);
        }
        self.state = CharacterState::Submitted;
        self.version += 1;
        Ok(())
    }

    pub fn approve_initial_version(&mut self) -> CoreEntityResult<()> {
        if self.state != CharacterState::Submitted {
            return Err(CoreEntityError::CharacterSheetNotSubmitted);
        }
        if self.initial_version_locked {
            return Err(CoreEntityError::CharacterSheetAlreadyLocked);
        }
        self.state = CharacterState::Approved;
        self.initial_version_locked = true;
        self.version += 1;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterSheetVersion {
    pub sheet_version_id: Option<CharacterSheetVersionId>,
    pub character_id: EntityId,
    pub version: u64,
    pub source_event_id: EntityId,
    pub sheet_json: Option<String>,
    pub locked: bool,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconsiderationState {
    Requested,
    Reviewed,
    Resolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconsiderationOutcome {
    Upheld,
    Corrected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reconsideration {
    pub reconsideration_id: ReconsiderationId,
    pub campaign_id: CampaignId,
    pub original_event_sequence: u64,
    pub requested_by: UserId,
    pub state: ReconsiderationState,
    pub outcome: Option<ReconsiderationOutcome>,
    pub event_chain: Vec<EntityId>,
    pub version: u64,
}

impl Reconsideration {
    pub fn requested(
        reconsideration_id: impl Into<String>,
        campaign_id: impl Into<String>,
        original_event_sequence: u64,
        requested_by: impl Into<String>,
        request_event_id: impl Into<String>,
    ) -> CoreEntityResult<Self> {
        if original_event_sequence == 0 {
            return Err(CoreEntityError::EventChainInvalid);
        }
        Ok(Self {
            reconsideration_id: ReconsiderationId::new(reconsideration_id)?,
            campaign_id: CampaignId::new(campaign_id)?,
            original_event_sequence,
            requested_by: UserId::new(requested_by)?,
            state: ReconsiderationState::Requested,
            outcome: None,
            event_chain: vec![
                EntityId::new(request_event_id).map_err(|_| CoreEntityError::InvalidIdentifier)?
            ],
            version: 1,
        })
    }

    pub fn append_review_event(&mut self, event_id: impl Into<String>) -> CoreEntityResult<()> {
        if self.state != ReconsiderationState::Requested {
            return Err(CoreEntityError::EventChainInvalid);
        }
        self.event_chain
            .push(EntityId::new(event_id).map_err(|_| CoreEntityError::InvalidIdentifier)?);
        self.state = ReconsiderationState::Reviewed;
        self.version += 1;
        Ok(())
    }

    pub fn resolve(
        &mut self,
        event_id: impl Into<String>,
        outcome: ReconsiderationOutcome,
    ) -> CoreEntityResult<()> {
        if self.state != ReconsiderationState::Reviewed {
            return Err(CoreEntityError::EventChainInvalid);
        }
        self.event_chain
            .push(EntityId::new(event_id).map_err(|_| CoreEntityError::InvalidIdentifier)?);
        self.state = ReconsiderationState::Resolved;
        self.outcome = Some(outcome);
        self.version += 1;
        Ok(())
    }
}

/// A child projection row derived from an immutable campaign-fork snapshot.
/// IDs are child-owned and deterministic; source IDs are retained only inside
/// the fork snapshot and materialization manifest for lineage/replay.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "row_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CampaignForkMaterializedRow {
    Scenario {
        scenario_id: String,
        ruleset_id: String,
        format_version: String,
        content_hash: String,
        document_json: String,
        visibility_label: String,
        visibility_subject: String,
    },
    Character {
        character_id: String,
        owner_user_id: String,
        display_name: String,
        state: String,
        initial_version_locked: bool,
        sheet_version_id: String,
        sheet_json: String,
        sheet_locked: bool,
        visibility_label: String,
        visibility_subject: String,
    },
    Session {
        session_id: String,
        room_id: String,
        scenario_id: String,
        state: String,
        active_scene_id: Option<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
        visibility_label: String,
        visibility_subject: String,
    },
    Scene {
        scene_id: String,
        session_id: String,
        scenario_id: String,
        room_id: String,
        scene_key: String,
        name: String,
        state: String,
        visibility_label: String,
        visibility_subject: String,
    },
    PublicEvent {
        fork_event_id: String,
        source_event_sequence: u64,
        source_event_type: String,
        source_resource_type: String,
        source_resource_id: String,
        source_payload_json: String,
        source_event_integrity_hash: String,
        visibility_label: String,
        visibility_subject: String,
    },
    DiscoveredClue {
        fork_clue_id: String,
        source_clue_id: String,
        importance: String,
        outcome: String,
        cost: Option<String>,
        visibility_label: String,
        visibility_subject: String,
    },
    NpcState {
        npc_state_id: String,
        source_npc_id: String,
        state_json: String,
        visibility_label: String,
        visibility_subject: String,
    },
    Combat {
        combat_id: String,
        session_id: String,
        status: String,
        round: u64,
        current_turn_index: u64,
        state_json: String,
        visibility_label: String,
        visibility_subject: String,
    },
    Chase {
        chase_id: String,
        session_id: String,
        status: String,
        range_band: u8,
        segment: u64,
        state_json: String,
        visibility_label: String,
        visibility_subject: String,
    },
    Conclusion {
        ending_event_id: String,
        session_id: String,
        ending_id: String,
        summary: String,
        ended_at_unix_ms: u64,
        visibility_label: String,
        visibility_subject: String,
    },
}

impl CampaignForkMaterializedRow {
    pub const fn projection_target_count(&self) -> usize {
        match self {
            Self::Character { .. } => 2,
            Self::Scenario { .. }
            | Self::Session { .. }
            | Self::Scene { .. }
            | Self::PublicEvent { .. }
            | Self::DiscoveredClue { .. }
            | Self::NpcState { .. }
            | Self::Combat { .. }
            | Self::Chase { .. }
            | Self::Conclusion { .. } => 1,
        }
    }
}

/// Versioned canonical payloads for P06 aggregates. The Event Store envelope
/// carries authority, visibility, provenance and command metadata; these
/// payloads carry only aggregate facts needed to rebuild projections.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CharacterCombatHealthUpdate {
    pub character_id: String,
    pub new_sheet_version_id: String,
    pub source_sheet_version: u64,
    pub source_character_version: u64,
    pub hp_before: u8,
    pub hp_after: u8,
    pub condition_before: String,
    pub condition_after: String,
}
