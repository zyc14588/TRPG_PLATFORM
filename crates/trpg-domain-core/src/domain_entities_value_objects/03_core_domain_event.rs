
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "event_type", content = "data")]
pub enum CoreDomainEvent {
    CampaignCreated {
        schema_version: u16,
        campaign_id: String,
        owner_user_id: String,
        authority_contract_id: String,
        authority_mode: String,
        authority_owner: String,
        title: String,
        room_id: String,
        room_name: String,
        created_at_unix_ms: u64,
    },
    CampaignInviteIssued {
        schema_version: u16,
        invite_id: String,
        campaign_id: String,
        invited_user_id: String,
        issued_by: String,
        role: MembershipRole,
        token_digest: String,
        expires_at_unix_ms: u64,
    },
    CampaignInviteAccepted {
        schema_version: u16,
        invite_id: String,
        campaign_id: String,
        user_id: String,
        role: MembershipRole,
        accepted_at_unix_ms: u64,
    },
    CharacterCreated {
        schema_version: u16,
        character_id: String,
        campaign_id: String,
        owner_user_id: String,
        display_name: String,
        sheet_version_id: String,
        sheet_json: String,
    },
    CharacterSubmitted {
        schema_version: u16,
        character_id: String,
    },
    CharacterInitialVersionApproved {
        schema_version: u16,
        character_id: String,
        reviewed_by: String,
    },
    ScenarioImported {
        schema_version: u16,
        scenario_id: String,
        campaign_id: String,
        ruleset_id: String,
        format_version: String,
        content_hash: String,
        document_json: String,
    },
    SessionStarted {
        schema_version: u16,
        session_id: String,
        campaign_id: String,
        room_id: String,
        scenario_id: String,
        scene_id: String,
        scene_key: String,
        scene_name: String,
        started_at_unix_ms: u64,
    },
    SessionStateChanged {
        schema_version: u16,
        session_id: String,
        from: SessionState,
        to: SessionState,
        changed_at_unix_ms: u64,
    },
    SceneSwitched {
        schema_version: u16,
        session_id: String,
        previous_scene_id: String,
        next_scene_id: String,
        next_scene_key: String,
        next_scene_name: String,
        switched_at_unix_ms: u64,
    },
    CampaignForkRecorded {
        schema_version: u16,
        fork_id: String,
        parent_campaign_id: String,
        child_campaign_id: String,
        source_session_id: String,
        snapshot_hash: String,
        child_snapshot_hash: String,
        copy_scopes: Vec<crate::fork_canon_lineage::CopyScope>,
        canonical_snapshot_json: String,
        reason: String,
    },
    CampaignForkMaterializationRecorded {
        schema_version: u16,
        fork_id: String,
        child_campaign_id: String,
        child_session_id: String,
        child_scenario_id: String,
        child_snapshot_hash: String,
        child_state_json: String,
        materialized_row_count: u64,
        batch_count: u64,
    },
    CampaignForkMaterialized {
        schema_version: u16,
        fork_id: String,
        child_campaign_id: String,
        batch_index: u64,
        batch_count: u64,
        rows: Vec<CampaignForkMaterializedRow>,
    },
    ReconsiderationRequested {
        schema_version: u16,
        reconsideration_id: String,
        campaign_id: String,
        original_event_sequence: u64,
        requested_by: String,
        reason: String,
    },
    ReconsiderationReviewed {
        schema_version: u16,
        reconsideration_id: String,
        review_event_id: String,
        review_summary: String,
    },
    ReconsiderationUpheld {
        schema_version: u16,
        reconsideration_id: String,
        resolution_event_id: String,
        original_event_sequence: u64,
        resolution: String,
    },
    ReconsiderationCorrected {
        schema_version: u16,
        reconsideration_id: String,
        resolution_event_id: String,
        original_event_sequence: u64,
        resolution: String,
        corrected_event_type: String,
        corrected_payload_json: String,
    },
    CombatStateRecorded {
        schema_version: u16,
        combat_id: String,
        campaign_id: String,
        session_id: String,
        status: String,
        round: u64,
        turn_index: u64,
        version: u64,
        state_json: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        character_health_updates: Vec<CharacterCombatHealthUpdate>,
    },
    ChaseStateRecorded {
        schema_version: u16,
        chase_id: String,
        campaign_id: String,
        session_id: String,
        status: String,
        range_band: u8,
        segment: u64,
        version: u64,
        state_json: String,
    },
    EndingRecorded {
        schema_version: u16,
        ending_event_id: String,
        campaign_id: String,
        session_id: String,
        ending_id: String,
        summary: String,
        ended_at_unix_ms: u64,
    },
    CharacterGrowthApplied {
        schema_version: u16,
        growth_event_id: String,
        campaign_id: String,
        session_id: String,
        ending_event_id: String,
        character_id: String,
        source_sheet_version_id: String,
        new_sheet_version_id: String,
        skill_name: String,
        skill_before: u8,
        improvement_check_roll: u8,
        increase_roll: Option<u8>,
        skill_after: u8,
        server_roll_id: String,
        increase_roll_id: Option<String>,
    },
}

impl CoreDomainEvent {
    pub const SCHEMA_VERSION: u16 = 1;

    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::CampaignCreated { .. } => "CampaignCreated",
            Self::CampaignInviteIssued { .. } => "CampaignInviteIssued",
            Self::CampaignInviteAccepted { .. } => "CampaignInviteAccepted",
            Self::CharacterCreated { .. } => "CharacterCreated",
            Self::CharacterSubmitted { .. } => "CharacterSubmitted",
            Self::CharacterInitialVersionApproved { .. } => "CharacterInitialVersionApproved",
            Self::ScenarioImported { .. } => "ScenarioImported",
            Self::SessionStarted { .. } => "SessionStarted",
            Self::SessionStateChanged { .. } => "SessionStateChanged",
            Self::SceneSwitched { .. } => "SceneSwitched",
            Self::CampaignForkRecorded { .. } => "CampaignForkRecorded",
            Self::CampaignForkMaterializationRecorded { .. } => {
                "CampaignForkMaterializationRecorded"
            }
            Self::CampaignForkMaterialized { .. } => "CampaignForkMaterialized",
            Self::ReconsiderationRequested { .. } => "ReconsiderationRequested",
            Self::ReconsiderationReviewed { .. } => "ReconsiderationReviewed",
            Self::ReconsiderationUpheld { .. } => "ReconsiderationUpheld",
            Self::ReconsiderationCorrected { .. } => "ReconsiderationCorrected",
            Self::CombatStateRecorded { .. } => "CombatStateRecorded",
            Self::ChaseStateRecorded { .. } => "ChaseStateRecorded",
            Self::EndingRecorded { .. } => "EndingRecorded",
            Self::CharacterGrowthApplied { .. } => "CharacterGrowthApplied",
        }
    }

    pub const fn schema_version(&self) -> u16 {
        match self {
            Self::CampaignCreated { schema_version, .. }
            | Self::CampaignInviteIssued { schema_version, .. }
            | Self::CampaignInviteAccepted { schema_version, .. }
            | Self::CharacterCreated { schema_version, .. }
            | Self::CharacterSubmitted { schema_version, .. }
            | Self::CharacterInitialVersionApproved { schema_version, .. }
            | Self::ScenarioImported { schema_version, .. }
            | Self::SessionStarted { schema_version, .. }
            | Self::SessionStateChanged { schema_version, .. }
            | Self::SceneSwitched { schema_version, .. }
            | Self::CampaignForkRecorded { schema_version, .. }
            | Self::CampaignForkMaterializationRecorded { schema_version, .. }
            | Self::CampaignForkMaterialized { schema_version, .. }
            | Self::ReconsiderationRequested { schema_version, .. }
            | Self::ReconsiderationReviewed { schema_version, .. }
            | Self::ReconsiderationUpheld { schema_version, .. }
            | Self::ReconsiderationCorrected { schema_version, .. }
            | Self::CombatStateRecorded { schema_version, .. }
            | Self::ChaseStateRecorded { schema_version, .. }
            | Self::EndingRecorded { schema_version, .. }
            | Self::CharacterGrowthApplied { schema_version, .. } => *schema_version,
        }
    }

    pub fn validate_schema_version(&self) -> CoreEntityResult<()> {
        if self.schema_version() == Self::SCHEMA_VERSION {
            Ok(())
        } else {
            Err(CoreEntityError::InvalidVersion)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryFact {
    fact_id: EntityId,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
    source_event_sequence: u64,
    confirmed: bool,
}

impl MemoryFact {
    pub fn confirmed(
        fact_id: impl Into<String>,
        evidence: &CommittedFactEvidence,
    ) -> DomainResult<Self> {
        let fact_id = EntityId::new(fact_id)?;
        if &fact_id != evidence.target_fact_id() {
            return Err(crate::ddd::DomainError::CommittedFactEvidenceInvalid);
        }
        Ok(Self {
            fact_id,
            source: evidence.source(),
            visibility: evidence.visibility().clone(),
            fact_provenance: evidence.fact_provenance().clone(),
            source_event_sequence: evidence.event_sequence(),
            confirmed: true,
        })
    }

    pub const fn is_confirmed(&self) -> bool {
        self.confirmed
    }

    pub fn fact_id(&self) -> &EntityId {
        &self.fact_id
    }

    pub const fn source(&self) -> FactSource {
        self.source
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn fact_provenance(&self) -> &FactProvenance {
        &self.fact_provenance
    }

    pub const fn source_event_sequence(&self) -> u64 {
        self.source_event_sequence
    }
}
