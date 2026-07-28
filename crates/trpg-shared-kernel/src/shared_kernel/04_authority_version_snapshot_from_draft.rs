
impl AuthorityVersionSnapshot {
    pub fn from_draft(draft: AuthorityVersionSnapshotDraft) -> KernelResult<Self> {
        Ok(Self {
            ruleset_version: EntityId::new(draft.ruleset_version)?,
            house_rules_version: EntityId::new(draft.house_rules_version)?,
            scenario_version: EntityId::new(draft.scenario_version)?,
            prompt_version: EntityId::new(draft.prompt_version)?,
            agent_pack_version: EntityId::new(draft.agent_pack_version)?,
            tool_schema_version: EntityId::new(draft.tool_schema_version)?,
            safety_profile_version: EntityId::new(draft.safety_profile_version)?,
            ai_provider_snapshot: EntityId::new(draft.ai_provider_snapshot)?,
            model_route_snapshot: EntityId::new(draft.model_route_snapshot)?,
            character_sheet_template_version: EntityId::new(
                draft.character_sheet_template_version,
            )?,
        })
    }

    pub fn ruleset_version(&self) -> &EntityId {
        &self.ruleset_version
    }

    pub fn house_rules_version(&self) -> &EntityId {
        &self.house_rules_version
    }

    pub fn scenario_version(&self) -> &EntityId {
        &self.scenario_version
    }

    pub fn prompt_version(&self) -> &EntityId {
        &self.prompt_version
    }

    pub fn agent_pack_version(&self) -> &EntityId {
        &self.agent_pack_version
    }

    pub fn tool_schema_version(&self) -> &EntityId {
        &self.tool_schema_version
    }

    pub fn safety_profile_version(&self) -> &EntityId {
        &self.safety_profile_version
    }

    pub fn ai_provider_snapshot(&self) -> &EntityId {
        &self.ai_provider_snapshot
    }

    pub fn model_route_snapshot(&self) -> &EntityId {
        &self.model_route_snapshot
    }

    pub fn character_sheet_template_version(&self) -> &EntityId {
        &self.character_sheet_template_version
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityContractDraft {
    pub contract_id: String,
    pub campaign_id: String,
    pub mode: AuthorityMode,
    pub authority_owner: String,
    pub version: u64,
    pub snapshot: AuthorityVersionSnapshotDraft,
    pub created_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityContract {
    contract_id: EntityId,
    campaign_id: EntityId,
    mode: AuthorityMode,
    authority_owner: EntityId,
    version: u64,
    snapshot: AuthorityVersionSnapshot,
    created_at_unix_ms: u64,
    locked: bool,
    change_policy: ChangePolicy,
}

impl AuthorityContract {
    pub fn new_locked(draft: AuthorityContractDraft) -> KernelResult<Self> {
        if draft.version == 0 || draft.created_at_unix_ms == 0 {
            return Err(TrpgError::AuthorityContractMutation);
        }
        Ok(Self {
            contract_id: EntityId::new(draft.contract_id)?,
            campaign_id: EntityId::new(draft.campaign_id)?,
            mode: draft.mode,
            authority_owner: EntityId::new(draft.authority_owner)?,
            version: draft.version,
            snapshot: AuthorityVersionSnapshot::from_draft(draft.snapshot)?,
            created_at_unix_ms: draft.created_at_unix_ms,
            locked: true,
            change_policy: ChangePolicy::ForkOnly,
        })
    }

    pub fn contract_id(&self) -> &EntityId {
        &self.contract_id
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn mode(&self) -> &AuthorityMode {
        &self.mode
    }

    pub fn authority_mode(&self) -> &AuthorityMode {
        &self.mode
    }

    pub fn authority_owner(&self) -> &EntityId {
        &self.authority_owner
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn snapshot(&self) -> &AuthorityVersionSnapshot {
        &self.snapshot
    }

    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }

    pub const fn is_locked(&self) -> bool {
        self.locked
    }

    pub const fn change_policy(&self) -> ChangePolicy {
        self.change_policy
    }

    pub fn binding(&self) -> KernelResult<AuthorityBinding> {
        AuthorityBinding::new(
            self.contract_id.as_str(),
            self.authority_owner.as_str(),
            self.mode.clone(),
            self.version,
        )
    }

    pub fn fork_with_draft(&self, draft: AuthorityContractDraft) -> KernelResult<Self> {
        if draft.campaign_id == self.campaign_id.as_str()
            || draft.contract_id == self.contract_id.as_str()
        {
            return Err(TrpgError::AuthorityContractMutation);
        }
        Self::new_locked(draft)
    }

    pub fn fork_for_child(
        &self,
        child_campaign_id: impl Into<String>,
        child_mode: AuthorityMode,
        child_owner: impl Into<String>,
    ) -> KernelResult<Self> {
        let child_campaign_id = child_campaign_id.into();
        self.fork_with_draft(AuthorityContractDraft {
            contract_id: format!("authority_contract_{child_campaign_id}_1"),
            campaign_id: child_campaign_id,
            mode: child_mode,
            authority_owner: child_owner.into(),
            version: 1,
            snapshot: AuthorityVersionSnapshotDraft {
                ruleset_version: self.snapshot.ruleset_version.to_string(),
                house_rules_version: self.snapshot.house_rules_version.to_string(),
                scenario_version: self.snapshot.scenario_version.to_string(),
                prompt_version: self.snapshot.prompt_version.to_string(),
                agent_pack_version: self.snapshot.agent_pack_version.to_string(),
                tool_schema_version: self.snapshot.tool_schema_version.to_string(),
                safety_profile_version: self.snapshot.safety_profile_version.to_string(),
                ai_provider_snapshot: self.snapshot.ai_provider_snapshot.to_string(),
                model_route_snapshot: self.snapshot.model_route_snapshot.to_string(),
                character_sheet_template_version: self
                    .snapshot
                    .character_sheet_template_version
                    .to_string(),
            },
            created_at_unix_ms: self.created_at_unix_ms.saturating_add(1),
        })
    }

    /// Authority is immutable inside a campaign. Existing call sites that try
    /// to change only mode/version are rejected; a legitimate fork must name a
    /// distinct child campaign through `fork_for_child` or `fork_with_draft`.
    pub fn fork(&self, _mode: AuthorityMode, _version: u64) -> KernelResult<Self> {
        Err(TrpgError::AuthorityContractMutation)
    }

    pub fn reject_in_place_authority_change(
        &self,
        attempted_mode: &AuthorityMode,
        attempted_owner: &EntityId,
    ) -> KernelResult<()> {
        if &self.mode != attempted_mode || &self.authority_owner != attempted_owner {
            return Err(TrpgError::AuthorityContractMutation);
        }
        Ok(())
    }

    pub fn validate_command<T>(&self, command: &CommandEnvelope<T>) -> KernelResult<()> {
        if !self.locked || self.change_policy != ChangePolicy::ForkOnly {
            return Err(TrpgError::AuthorityContractMutation);
        }
        if self.mode != command.authority_mode {
            return Err(TrpgError::AuthorityViolation);
        }
        let context = command.authenticated_context();
        if context.resource().campaign_id() != &self.campaign_id {
            return Err(TrpgError::CampaignScopeMismatch);
        }
        if context.authority().contract_id() != &self.contract_id {
            return Err(TrpgError::AuthorityContractMutation);
        }
        if context.authority().authority_owner() != &self.authority_owner {
            return Err(TrpgError::AuthorityOwnerMismatch);
        }
        if context.authority().authority_mode() != &self.mode {
            return Err(TrpgError::AuthorityViolation);
        }
        if context.authority().contract_version() != self.version
            || command.authority_contract_version != self.version
        {
            return Err(TrpgError::AuthorityContractVersionConflict);
        }
        if command.actor.role() == &ActorRole::HumanKeeper
            && command.actor.id() != &self.authority_owner
        {
            return Err(TrpgError::AuthorityOwnerMismatch);
        }
        validate_command_envelope(command)
    }
}

/// Canonical, process-local view of persisted Authority Contracts. A campaign
/// can be registered exactly once; changing mode, owner, version, or contract
/// id requires a distinct child campaign fork.
#[derive(Clone, Debug, Default)]
pub struct AuthorityRegistry {
    contracts_by_campaign: HashMap<EntityId, AuthorityContract>,
}

impl AuthorityRegistry {
    pub fn register(&mut self, contract: AuthorityContract) -> KernelResult<()> {
        match self.contracts_by_campaign.get(contract.campaign_id()) {
            Some(existing) if existing == &contract => Ok(()),
            Some(_) => Err(TrpgError::AuthorityContractMutation),
            None => {
                self.contracts_by_campaign
                    .insert(contract.campaign_id().clone(), contract);
                Ok(())
            }
        }
    }

    pub fn from_contracts(
        contracts: impl IntoIterator<Item = AuthorityContract>,
    ) -> KernelResult<Self> {
        let mut registry = Self::default();
        for contract in contracts {
            registry.register(contract)?;
        }
        Ok(registry)
    }

    pub fn contract_for(&self, campaign_id: &EntityId) -> KernelResult<&AuthorityContract> {
        self.contracts_by_campaign
            .get(campaign_id)
            .ok_or(TrpgError::AuthorityViolation)
    }

    pub fn validate_command<T>(
        &self,
        command: &CommandEnvelope<T>,
    ) -> KernelResult<&AuthorityContract> {
        let campaign_id = command.authenticated_context().resource().campaign_id();
        let contract = self.contract_for(campaign_id)?;
        contract.validate_command(command)?;
        Ok(contract)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandEnvelope<T> {
    pub command_id: EntityId,
    pub idempotency_key: String,
    pub expected_version: u64,
    pub actor: Actor,
    pub authority_mode: AuthorityMode,
    pub authority_contract_version: u64,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
    pub write_path: FormalWritePath,
    pub payload: T,
    authenticated_context: AuthenticatedCommandContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandMetadata {
    pub command_id: EntityId,
    pub idempotency_key: String,
    pub expected_version: u64,
    pub authority_mode: AuthorityMode,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
    pub write_path: FormalWritePath,
    pub authenticated_context: AuthenticatedCommandContext,
}

impl<T> CommandEnvelope<T> {
    pub fn new(payload: T, metadata: CommandMetadata) -> Self {
        let actor = metadata.authenticated_context.actor().clone();
        let authority_contract_version = metadata
            .authenticated_context
            .authority()
            .contract_version();
        Self {
            command_id: metadata.command_id,
            idempotency_key: metadata.idempotency_key,
            expected_version: metadata.expected_version,
            actor,
            authority_mode: metadata.authority_mode,
            authority_contract_version,
            visibility: metadata.visibility,
            fact_provenance: metadata.fact_provenance,
            correlation_id: metadata.correlation_id,
            causation_id: metadata.causation_id,
            write_path: metadata.write_path,
            payload,
            authenticated_context: metadata.authenticated_context,
        }
    }

    pub fn authenticated_context(&self) -> &AuthenticatedCommandContext {
        &self.authenticated_context
    }
}
