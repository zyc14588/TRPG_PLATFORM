
impl Visibility {
    pub fn new(label: VisibilityLabel) -> Self {
        Self { label }
    }

    pub fn private_to_player(player_id: EntityId) -> Self {
        Self {
            label: VisibilityLabel::PrivateToPlayer(player_id),
        }
    }

    pub fn private_to_group(group_id: EntityId) -> Self {
        Self {
            label: VisibilityLabel::PrivateToGroup(group_id),
        }
    }

    pub fn investigator_private(player_id: EntityId) -> Self {
        Self {
            label: VisibilityLabel::InvestigatorPrivate(player_id),
        }
    }

    pub fn try_from_parts(label: &str, subject_id: Option<&str>) -> KernelResult<Self> {
        let kind = VisibilityKind::try_from(label)?;
        match (kind.requires_subject(), subject_id) {
            (true, Some(subject)) => {
                let subject = EntityId::new(subject)?;
                Ok(match kind {
                    VisibilityKind::PrivateToPlayer => Self::private_to_player(subject),
                    VisibilityKind::PrivateToGroup => Self::private_to_group(subject),
                    VisibilityKind::InvestigatorPrivate => Self::investigator_private(subject),
                    _ => unreachable!("subject-bearing visibility kind"),
                })
            }
            (false, None) => Ok(Self::new(VisibilityLabel::try_from(label)?)),
            _ => Err(TrpgError::VisibilityDenied),
        }
    }

    pub fn label(&self) -> &VisibilityLabel {
        &self.label
    }

    pub fn player_id(&self) -> Option<&EntityId> {
        self.label
            .is_private_to_player()
            .then_some(self.label.subject_id())
            .flatten()
    }

    pub fn group_id(&self) -> Option<&EntityId> {
        self.label
            .is_private_to_group()
            .then_some(self.label.subject_id())
            .flatten()
    }

    /// The audience identity carried by targeted visibility labels.
    /// Callers that persist or hash visibility metadata must use this method
    /// rather than assuming that every target is a player.
    pub fn subject_id(&self) -> Option<&EntityId> {
        self.label.subject_id()
    }

    pub fn is_well_formed(&self) -> bool {
        true
    }

    pub fn can_view(&self, principal: &PrincipalScope) -> bool {
        match self.label.kind() {
            VisibilityKind::Public => true,
            VisibilityKind::PartyVisible | VisibilityKind::SpectatorHidden => {
                principal.is_party_audience()
            }
            VisibilityKind::KeeperOnly => principal.is_keeper() || principal.is_system(),
            VisibilityKind::PrivateToPlayer | VisibilityKind::InvestigatorPrivate => {
                principal.is_keeper()
                    || principal.is_system()
                    || self
                        .subject_id()
                        .is_some_and(|subject| principal.matches_player(subject))
            }
            VisibilityKind::PrivateToGroup => {
                principal.is_keeper()
                    || principal.is_system()
                    || self
                        .subject_id()
                        .is_some_and(|subject| principal.matches_group(subject))
            }
            VisibilityKind::SpectatorVisible => {
                principal.is_spectator() || principal.is_party_audience()
            }
            VisibilityKind::AiInternal => principal.is_ai_runtime() || principal.is_system(),
            VisibilityKind::SystemOnly | VisibilityKind::SystemPrivate => principal.is_system(),
        }
    }

    /// Computes the audience intersection of two source values. The result is
    /// never broader than either source. Incomparable private targets collapse
    /// to keeper/system or system-only rather than depending on input order.
    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            label: intersect_visibility_labels(&self.label, &other.label),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VisibilityWireValue {
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subject_id: Option<String>,
}

impl Serialize for Visibility {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        VisibilityWireValue {
            label: self.label.as_str().to_owned(),
            subject_id: self.subject_id().map(ToString::to_string),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Visibility {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = VisibilityWireValue::deserialize(deserializer)?;
        Self::try_from_parts(&value.label, value.subject_id.as_deref())
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrincipalCapability {
    PartyMember,
    Keeper,
    Spectator,
    AiRuntime,
    System,
}

/// Lossless authenticated principal claims. Unlike the compatibility enum
/// variants below, this value can simultaneously represent one user/player,
/// multiple groups and characters, spectator status, and trusted workload
/// capabilities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrincipalClaims {
    user_id: EntityId,
    player_id: Option<EntityId>,
    group_ids: Vec<EntityId>,
    character_ids: Vec<EntityId>,
    capabilities: Vec<PrincipalCapability>,
}

impl PrincipalClaims {
    pub fn new(user_id: impl Into<String>) -> KernelResult<Self> {
        Ok(Self {
            user_id: EntityId::new(user_id)?,
            player_id: None,
            group_ids: Vec::new(),
            character_ids: Vec::new(),
            capabilities: Vec::new(),
        })
    }

    pub fn with_player(mut self, player_id: impl Into<String>) -> KernelResult<Self> {
        self.player_id = Some(EntityId::new(player_id)?);
        Ok(self)
    }

    pub fn with_group(mut self, group_id: impl Into<String>) -> KernelResult<Self> {
        push_unique(&mut self.group_ids, EntityId::new(group_id)?);
        Ok(self)
    }

    pub fn with_character(mut self, character_id: impl Into<String>) -> KernelResult<Self> {
        push_unique(&mut self.character_ids, EntityId::new(character_id)?);
        Ok(self)
    }

    pub fn with_capability(mut self, capability: PrincipalCapability) -> Self {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
        self
    }

    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }

    pub fn player_id(&self) -> Option<&EntityId> {
        self.player_id.as_ref()
    }

    pub fn group_ids(&self) -> &[EntityId] {
        &self.group_ids
    }

    pub fn character_ids(&self) -> &[EntityId] {
        &self.character_ids
    }

    pub fn has_capability(&self, capability: PrincipalCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrincipalScope {
    Public,
    PartyMember,
    Keeper,
    Player(EntityId),
    GroupMember(EntityId),
    Spectator,
    System,
    Claims(PrincipalClaims),
}

impl PrincipalScope {
    fn is_system(&self) -> bool {
        matches!(self, Self::System)
            || matches!(self, Self::Claims(claims) if claims.has_capability(PrincipalCapability::System))
    }

    fn is_keeper(&self) -> bool {
        matches!(self, Self::Keeper)
            || matches!(self, Self::Claims(claims) if claims.has_capability(PrincipalCapability::Keeper))
    }

    fn is_ai_runtime(&self) -> bool {
        matches!(self, Self::Claims(claims) if claims.has_capability(PrincipalCapability::AiRuntime))
    }

    fn is_spectator(&self) -> bool {
        matches!(self, Self::Spectator)
            || matches!(self, Self::Claims(claims) if claims.has_capability(PrincipalCapability::Spectator))
    }

    fn is_party_audience(&self) -> bool {
        matches!(
            self,
            Self::PartyMember
                | Self::Player(_)
                | Self::GroupMember(_)
                | Self::Keeper
                | Self::System
        ) || matches!(self, Self::Claims(claims) if claims.player_id.is_some()
            || !claims.group_ids.is_empty()
            || claims.has_capability(PrincipalCapability::PartyMember)
            || claims.has_capability(PrincipalCapability::Keeper)
            || claims.has_capability(PrincipalCapability::System))
    }

    fn matches_player(&self, subject: &EntityId) -> bool {
        matches!(self, Self::Player(player_id) if player_id == subject)
            || matches!(self, Self::Claims(claims) if claims.player_id.as_ref() == Some(subject))
    }

    fn matches_group(&self, subject: &EntityId) -> bool {
        matches!(self, Self::GroupMember(group_id) if group_id == subject)
            || matches!(self, Self::Claims(claims) if claims.group_ids.contains(subject))
    }
}

fn push_unique(values: &mut Vec<EntityId>, value: EntityId) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn intersect_visibility_labels(left: &VisibilityLabel, right: &VisibilityLabel) -> VisibilityLabel {
    use VisibilityKind::*;

    if left == right {
        return left.clone();
    }
    match (left.kind(), right.kind()) {
        (Public, _) => right.clone(),
        (_, Public) => left.clone(),
        (SystemPrivate, _) | (_, SystemPrivate) => VisibilityLabel::SystemPrivate,
        (SystemOnly, _) | (_, SystemOnly) => VisibilityLabel::SystemOnly,
        (AiInternal, AiInternal) => VisibilityLabel::AiInternal,
        (AiInternal, _) | (_, AiInternal) => VisibilityLabel::SystemOnly,
        (KeeperOnly, _) | (_, KeeperOnly) => VisibilityLabel::KeeperOnly,
        (PrivateToPlayer | InvestigatorPrivate, PrivateToPlayer | InvestigatorPrivate)
            if left.subject_id() == right.subject_id() =>
        {
            VisibilityLabel::targeted(
                if matches!(left.kind(), InvestigatorPrivate)
                    || matches!(right.kind(), InvestigatorPrivate)
                {
                    InvestigatorPrivate
                } else {
                    PrivateToPlayer
                },
                left.subject_id()
                    .expect("targeted label has a subject")
                    .clone(),
            )
        }
        (PrivateToGroup, PrivateToGroup) if left.subject_id() == right.subject_id() => left.clone(),
        (
            PrivateToPlayer | PrivateToGroup | InvestigatorPrivate,
            PrivateToPlayer | PrivateToGroup | InvestigatorPrivate,
        ) => VisibilityLabel::KeeperOnly,
        (PrivateToPlayer | PrivateToGroup | InvestigatorPrivate, _)
        | (_, PrivateToPlayer | PrivateToGroup | InvestigatorPrivate) => {
            if left.kind().requires_subject() {
                left.clone()
            } else {
                right.clone()
            }
        }
        (SpectatorVisible, _) => right.clone(),
        (_, SpectatorVisible) => left.clone(),
        (PartyVisible, SpectatorHidden) | (SpectatorHidden, PartyVisible) => {
            VisibilityLabel::SpectatorHidden
        }
        (PartyVisible, PartyVisible) => VisibilityLabel::PartyVisible,
        (SpectatorHidden, SpectatorHidden) => VisibilityLabel::SpectatorHidden,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum ProvenanceKind {
    UserStatement,
    HumanKeeperStatement,
    RulesEngineDecision,
    ToolResult,
    AgentProposal,
    ImportedSource,
    SystemFixture,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactProvenance {
    pub kind: ProvenanceKind,
    pub reference: EntityId,
    pub recorded_by: EntityId,
}
