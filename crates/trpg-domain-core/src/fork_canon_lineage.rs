use sha2::{Digest, Sha256};

use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{AuthorityMode, DomainError, DomainResult, EntityId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CanonStatus {
    Canon,
    NonCanon,
    WhatIf,
    EmergencyFork,
    Archived,
    Frozen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CopyScope {
    CharacterState,
    PublicEvents,
    DiscoveredClues,
    WorldState,
    NpcState,
    SceneState,
    CombatState,
    ChaseState,
    ConclusionState,
    KeeperNotes,
    HiddenClues,
    PrivateMessages,
    AiInternalMemory,
}

impl CopyScope {
    pub const fn is_private(self) -> bool {
        matches!(
            self,
            Self::KeeperNotes | Self::HiddenClues | Self::PrivateMessages | Self::AiInternalMemory
        )
    }
}

pub const DEFAULT_PUBLIC_COPY_SCOPES: &[CopyScope] = &[
    CopyScope::CharacterState,
    CopyScope::PublicEvents,
    CopyScope::DiscoveredClues,
    CopyScope::WorldState,
    CopyScope::NpcState,
    CopyScope::SceneState,
    CopyScope::CombatState,
    CopyScope::ChaseState,
    CopyScope::ConclusionState,
];

pub const PRIVATE_COPY_SCOPES: &[CopyScope] = &[
    CopyScope::KeeperNotes,
    CopyScope::HiddenClues,
    CopyScope::PrivateMessages,
    CopyScope::AiInternalMemory,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignForkSnapshot {
    pub source_campaign_id: EntityId,
    pub source_session_id: EntityId,
    pub canonical_state_json: String,
    pub snapshot_hash: String,
}

impl CampaignForkSnapshot {
    pub fn verified(
        source_campaign_id: impl Into<String>,
        source_session_id: impl Into<String>,
        canonical_state_json: impl Into<String>,
        snapshot_hash: impl Into<String>,
    ) -> DomainResult<Self> {
        let canonical_state_json = canonical_state_json.into();
        let snapshot_hash = snapshot_hash.into();
        if canonical_state_json.trim().is_empty()
            || !valid_snapshot_hash(&snapshot_hash)
            || calculate_snapshot_hash(&canonical_state_json) != snapshot_hash
        {
            return Err(DomainError::PolicyDenied);
        }
        Ok(Self {
            source_campaign_id: EntityId::new(source_campaign_id)?,
            source_session_id: EntityId::new(source_session_id)?,
            canonical_state_json,
            snapshot_hash,
        })
    }
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
    pub canon_status: CanonStatus,
    pub copy_scopes: Vec<CopyScope>,
}

impl CampaignForkRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_campaign_id: impl Into<String>,
        fork_source_session_id: impl Into<String>,
        new_campaign_id: impl Into<String>,
        new_authority_mode: AuthorityMode,
        new_authority_owner: impl Into<String>,
        fork_reason: impl Into<String>,
        snapshot_hash: impl Into<String>,
    ) -> DomainResult<Self> {
        let source_campaign_id = EntityId::new(source_campaign_id)?;
        let new_campaign_id = EntityId::new(new_campaign_id)?;
        let fork_reason = fork_reason.into();
        let snapshot_hash = snapshot_hash.into();
        if source_campaign_id == new_campaign_id
            || fork_reason.trim().is_empty()
            || fork_reason.len() > 512
            || !valid_snapshot_hash(&snapshot_hash)
        {
            return Err(DomainError::PolicyDenied);
        }
        Ok(Self {
            source_campaign_id,
            fork_source_session_id: EntityId::new(fork_source_session_id)?,
            new_campaign_id,
            new_authority_mode,
            new_authority_owner: EntityId::new(new_authority_owner)?,
            fork_reason: fork_reason.trim().to_owned(),
            snapshot_hash,
            canon_status: CanonStatus::WhatIf,
            copy_scopes: DEFAULT_PUBLIC_COPY_SCOPES.to_vec(),
        })
    }

    pub fn with_scope(
        mut self,
        canon_status: CanonStatus,
        copy_scopes: Vec<CopyScope>,
    ) -> DomainResult<Self> {
        if matches!(
            canon_status,
            CanonStatus::Canon | CanonStatus::Archived | CanonStatus::Frozen
        ) || copy_scopes.is_empty()
            || has_duplicate_scopes(&copy_scopes)
        {
            return Err(DomainError::PolicyDenied);
        }
        self.canon_status = canon_status;
        self.copy_scopes = copy_scopes;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignFork {
    pub parent_campaign_id: EntityId,
    pub child_campaign_id: EntityId,
    pub child_authority_contract: DomainAuthorityContract,
    pub canon_status: CanonStatus,
    pub parent_unchanged: bool,
    pub copied_scopes: Vec<CopyScope>,
    pub excluded_private_scopes: Vec<CopyScope>,
    pub copied_snapshot: CampaignForkSnapshot,
}

pub fn fork_campaign(
    parent_contract: &DomainAuthorityContract,
    request: &CampaignForkRequest,
    snapshot: &CampaignForkSnapshot,
    authorized_private_scopes: &[CopyScope],
) -> DomainResult<CampaignFork> {
    if parent_contract.campaign_id() != &request.source_campaign_id
        || snapshot.source_campaign_id != request.source_campaign_id
        || snapshot.source_session_id != request.fork_source_session_id
        || snapshot.snapshot_hash != request.snapshot_hash
        || calculate_snapshot_hash(&snapshot.canonical_state_json) != request.snapshot_hash
    {
        return Err(DomainError::AuthorityViolation);
    }
    if authorized_private_scopes
        .iter()
        .any(|scope| !scope.is_private())
        || request
            .copy_scopes
            .iter()
            .filter(|scope| scope.is_private())
            .any(|scope| !authorized_private_scopes.contains(scope))
    {
        return Err(DomainError::VisibilityDenied);
    }

    let child_authority_contract = parent_contract.fork_for_child(
        request.new_campaign_id.as_str(),
        request.new_authority_mode.clone(),
        request.new_authority_owner.as_str(),
    )?;
    let excluded_private_scopes = PRIVATE_COPY_SCOPES
        .iter()
        .copied()
        .filter(|scope| !request.copy_scopes.contains(scope))
        .collect();

    Ok(CampaignFork {
        parent_campaign_id: parent_contract.campaign_id().clone(),
        child_campaign_id: request.new_campaign_id.clone(),
        child_authority_contract,
        canon_status: request.canon_status,
        parent_unchanged: true,
        copied_scopes: request.copy_scopes.clone(),
        excluded_private_scopes,
        copied_snapshot: snapshot.clone(),
    })
}

pub fn calculate_snapshot_hash(canonical_state_json: &str) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(canonical_state_json.as_bytes())
    )
}

fn valid_snapshot_hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn has_duplicate_scopes(scopes: &[CopyScope]) -> bool {
    scopes
        .iter()
        .enumerate()
        .any(|(index, scope)| scopes[index + 1..].contains(scope))
}
