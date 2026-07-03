use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{AuthorityMode, DomainError, DomainResult, EntityId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonStatus {
    Canon,
    NonCanon,
    WhatIf,
    EmergencyFork,
    Archived,
    Frozen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyScope {
    CharacterState,
    PublicEvents,
    DiscoveredClues,
    WorldState,
    NpcState,
    SceneState,
    KeeperNotes,
    HiddenClues,
    PrivateMessages,
    AiInternalMemory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignForkRequest {
    pub source_campaign_id: EntityId,
    pub fork_source_session_id: EntityId,
    pub new_campaign_id: EntityId,
    pub new_authority_mode: AuthorityMode,
    pub new_authority_owner: EntityId,
    pub fork_reason: String,
    pub snapshot_hash: String,
}

impl CampaignForkRequest {
    pub fn new(
        source_campaign_id: impl Into<String>,
        fork_source_session_id: impl Into<String>,
        new_campaign_id: impl Into<String>,
        new_authority_mode: AuthorityMode,
        new_authority_owner: impl Into<String>,
        fork_reason: impl Into<String>,
        snapshot_hash: impl Into<String>,
    ) -> DomainResult<Self> {
        Ok(Self {
            source_campaign_id: EntityId::new(source_campaign_id)?,
            fork_source_session_id: EntityId::new(fork_source_session_id)?,
            new_campaign_id: EntityId::new(new_campaign_id)?,
            new_authority_mode,
            new_authority_owner: EntityId::new(new_authority_owner)?,
            fork_reason: fork_reason.into(),
            snapshot_hash: snapshot_hash.into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignFork {
    pub parent_campaign_id: EntityId,
    pub child_campaign_id: EntityId,
    pub child_authority_contract: DomainAuthorityContract,
    pub canon_status: CanonStatus,
    pub parent_unchanged: bool,
    pub copied_by_default: Vec<CopyScope>,
    pub requires_explicit_permission: Vec<CopyScope>,
}

pub fn fork_campaign(
    parent_contract: &DomainAuthorityContract,
    request: &CampaignForkRequest,
) -> DomainResult<CampaignFork> {
    if parent_contract.campaign_id != request.source_campaign_id {
        return Err(DomainError::AuthorityViolation);
    }

    let child_authority_contract = parent_contract.fork_for_child(
        request.new_campaign_id.as_str(),
        request.new_authority_mode.clone(),
        request.new_authority_owner.as_str(),
    )?;

    Ok(CampaignFork {
        parent_campaign_id: parent_contract.campaign_id.clone(),
        child_campaign_id: request.new_campaign_id.clone(),
        child_authority_contract,
        canon_status: CanonStatus::WhatIf,
        parent_unchanged: true,
        copied_by_default: vec![
            CopyScope::CharacterState,
            CopyScope::PublicEvents,
            CopyScope::DiscoveredClues,
            CopyScope::WorldState,
            CopyScope::NpcState,
            CopyScope::SceneState,
        ],
        requires_explicit_permission: vec![
            CopyScope::KeeperNotes,
            CopyScope::HiddenClues,
            CopyScope::PrivateMessages,
            CopyScope::AiInternalMemory,
        ],
    })
}
