
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityContractSnapshot {
    pub contract_id: String,
    pub authority_mode: String,
    pub authority_owner: String,
    pub ruleset_version: String,
    pub house_rules_version: String,
    pub scenario_version: String,
    pub prompt_version: String,
    pub agent_pack_version: String,
    pub tool_schema_version: String,
    pub safety_profile_version: String,
    pub ai_provider_snapshot: String,
    pub model_route_snapshot: String,
    pub character_sheet_template_version: String,
}

impl AuthorityContractSnapshot {
    fn validate(
        &self,
        campaign_id: &str,
        metadata: &CoreCommandMetadata,
    ) -> Result<(), CoreDomainRepositoryError> {
        let expected_wire_mode = match self.authority_mode.as_str() {
            "HUMAN_KP" => "human_kp",
            "AI_KP" => "ai_kp",
            _ => return Err(CoreDomainRepositoryError::InvalidInput("authority_mode")),
        };
        let required = [
            self.contract_id.as_str(),
            self.authority_owner.as_str(),
            self.ruleset_version.as_str(),
            self.house_rules_version.as_str(),
            self.scenario_version.as_str(),
            self.prompt_version.as_str(),
            self.agent_pack_version.as_str(),
            self.tool_schema_version.as_str(),
            self.safety_profile_version.as_str(),
            self.ai_provider_snapshot.as_str(),
            self.model_route_snapshot.as_str(),
            self.character_sheet_template_version.as_str(),
        ];
        if required.iter().any(|value| value.trim().is_empty())
            || expected_wire_mode != metadata.authority_mode
            || self.contract_id != metadata.authority_contract_id
            || self.authority_owner != metadata.authority_owner
            || metadata.authority_contract_version != 1
            || campaign_id.trim().is_empty()
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "authority_contract",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateCampaignRequest {
    pub campaign_id: String,
    pub owner_user_id: String,
    pub title: String,
    pub room_id: String,
    pub room_name: String,
    pub created_at_unix_ms: u64,
    pub authority: AuthorityContractSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueInviteRequest {
    pub invite_id: String,
    pub campaign_id: String,
    pub invited_user_id: String,
    pub role: MembershipRole,
    pub expires_at_unix_ms: u64,
}

pub struct IssuedCampaignInvite {
    pub invite_id: String,
    pub raw_token: String,
    pub expires_at_unix_ms: u64,
    pub persisted: PersistedCommit,
}

impl fmt::Debug for IssuedCampaignInvite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedCampaignInvite")
            .field("invite_id", &self.invite_id)
            .field("raw_token", &"[REDACTED]")
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .field("persisted", &self.persisted)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptInviteRequest {
    pub campaign_id: String,
    pub invite_id: String,
    pub accepting_user_id: String,
    pub raw_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateCharacterRequest {
    pub character_id: String,
    pub campaign_id: String,
    pub owner_user_id: String,
    pub display_name: String,
    pub sheet_version_id: String,
    pub sheet_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PlayerActionIntentRecord {
    Investigation {
        skill_name: String,
        clue_id: String,
        clue_importance: String,
        adjustment: String,
    },
    SanityCheck {
        success_loss: u8,
        failure_loss: u8,
        day_key: String,
    },
}

impl PlayerActionIntentRecord {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Investigation { .. } => "INVESTIGATION",
            Self::SanityCheck { .. } => "SANITY_CHECK",
        }
    }

    fn validate(&self) -> Result<(), CoreDomainRepositoryError> {
        match self {
            Self::Investigation {
                skill_name,
                clue_id,
                clue_importance,
                adjustment,
            } => {
                if skill_name.trim().is_empty()
                    || skill_name.len() > 128
                    || EntityId::new(clue_id).is_err()
                    || !matches!(clue_importance.as_str(), "CORE" | "OPTIONAL")
                    || !matches!(adjustment.as_str(), "NONE" | "BONUS" | "PENALTY")
                {
                    return Err(CoreDomainRepositoryError::InvalidInput(
                        "investigation_intent",
                    ));
                }
            }
            Self::SanityCheck {
                success_loss,
                failure_loss,
                day_key,
            } => {
                if day_key.trim().is_empty()
                    || day_key.len() > 128
                    || success_loss > failure_loss
                    || *failure_loss > 99
                {
                    return Err(CoreDomainRepositoryError::InvalidInput("sanity_intent"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmitPlayerActionRequest {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub scene_id: String,
    pub submitted_by: String,
    pub submitted_at_unix_ms: u64,
    pub intent: PlayerActionIntentRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPlayerActionRecord {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub scene_id: String,
    pub submitted_by: String,
    pub intent: PlayerActionIntentRecord,
    pub character_sheet_json: String,
    pub character_sheet_version: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerActionHeader {
    pub action_kind: String,
    pub submitted_by: String,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerActionDiceRecord {
    pub roll_id: String,
    pub target_value: u8,
    pub rolled_value: u8,
    pub success_level: String,
    pub selected_tens_digit: u8,
    pub ones_digit: u8,
    pub adjustment: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvestigationExecutionRecord {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub decision_id: String,
    pub tool_execution_id: String,
    pub confirmed_by: String,
    pub resolved_at_unix_ms: u64,
    pub dice: PlayerActionDiceRecord,
    pub skill_name: String,
    pub clue_record_id: String,
    pub clue_id: String,
    pub clue_importance: String,
    pub clue_outcome: String,
    pub clue_cost: Option<String>,
    pub revealed_to_party: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanityExecutionRecord {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub decision_id: String,
    pub tool_execution_id: String,
    pub confirmed_by: String,
    pub resolved_at_unix_ms: u64,
    pub dice: PlayerActionDiceRecord,
    pub sanity_event_id: String,
    pub sheet_version_id: String,
    pub day_key: String,
    pub day_start_sanity: u8,
    pub sanity_before: u8,
    pub sanity_after: u8,
    pub sanity_loss: u8,
    pub day_loss: u8,
    pub indefinite_threshold: u8,
    pub madness_state: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportScenarioRequest {
    pub scenario_id: String,
    pub campaign_id: String,
    pub ruleset_id: String,
    pub format_version: String,
    pub content_hash: String,
    pub document_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartSessionRequest {
    pub session_id: String,
    pub campaign_id: String,
    pub room_id: String,
    pub scenario_id: String,
    pub scene_id: String,
    pub scene_key: String,
    pub scene_name: String,
    pub started_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchSceneRequest {
    pub session_id: String,
    pub campaign_id: String,
    pub next_scene_id: String,
    pub next_scene_key: String,
    pub next_scene_name: String,
    pub switched_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionProjectionRebuildReport {
    pub campaign_id: String,
    pub replayed_events: usize,
    pub restored_sessions: i64,
    pub restored_scenes: i64,
    pub last_event_sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P08ProjectionRebuildReport {
    pub campaign_id: String,
    pub replayed_events: usize,
    pub combat_states: i64,
    pub chase_states: i64,
    pub reconsiderations: i64,
    pub campaign_forks: i64,
    pub fork_materializations: i64,
    pub fork_public_events: i64,
    pub fork_clues: i64,
    pub fork_npc_states: i64,
    pub gameplay_roll_consumptions: i64,
    pub ending_events: i64,
    pub growth_events: i64,
    pub last_event_sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordCampaignForkRequest {
    pub fork_id: String,
    pub parent_campaign_id: String,
    pub child_campaign_id: String,
    pub source_session_id: String,
    pub snapshot_hash: String,
    pub reason: String,
    pub copy_scopes: Vec<CopyScope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignForkSnapshotPreview {
    pub canonical_snapshot_json: String,
    pub snapshot_hash: String,
    pub copy_scopes: Vec<CopyScope>,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotEnvelope {
    state: ForkSnapshotState,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotState {
    source_campaign_id: String,
    source_session_id: String,
    session_state: ForkSnapshotSession,
    character_state: Vec<ForkSnapshotCharacter>,
    public_events: Vec<ForkSnapshotPublicEvent>,
    discovered_clues: Vec<ForkSnapshotClue>,
    scene_state: Vec<ForkSnapshotScene>,
    world_state: ForkSnapshotWorld,
    combat_state: Vec<ForkSnapshotCombat>,
    chase_state: Vec<ForkSnapshotChase>,
    conclusion_state: Vec<ForkSnapshotConclusion>,
    npc_state: Vec<ForkSnapshotNpcState>,
}
