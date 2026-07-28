
impl FactProvenance {
    pub fn new(
        kind: ProvenanceKind,
        reference: impl Into<String>,
        recorded_by: impl Into<String>,
    ) -> KernelResult<Self> {
        Ok(Self {
            kind,
            reference: EntityId::new(reference)?,
            recorded_by: EntityId::new(recorded_by)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityMode {
    HumanKp,
    AiKp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorRole {
    ServerOwner,
    CampaignOwner,
    HumanKeeper,
    AiKeeper,
    Investigator,
    Moderator,
    Spectator,
    Workflow,
    RulesEngine,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkloadRole {
    ApiServer,
    RealtimeServer,
    AgentWorker,
    WorkflowEngine,
    RulesEngine,
    AuditWriter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentClass {
    AiKeeperOrchestrator,
    KeeperCopilot,
    AtmosphereWriter,
    MemoryCurator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorOrigin {
    UserSession {
        session_id: EntityId,
    },
    Workload {
        role: WorkloadRole,
    },
    AgentRun {
        run_id: EntityId,
        class: AgentClass,
        campaign_id: EntityId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    id: EntityId,
    role: ActorRole,
    origin: ActorOrigin,
}

impl Actor {
    pub fn authenticated_user(
        id: impl Into<String>,
        role: ActorRole,
        session_id: impl Into<String>,
    ) -> KernelResult<Self> {
        if !matches!(
            role,
            ActorRole::ServerOwner
                | ActorRole::CampaignOwner
                | ActorRole::HumanKeeper
                | ActorRole::Investigator
                | ActorRole::Moderator
                | ActorRole::Spectator
        ) {
            return Err(TrpgError::InternalIdentityInvalid);
        }
        Ok(Self {
            id: EntityId::new(id)?,
            role,
            origin: ActorOrigin::UserSession {
                session_id: EntityId::new(session_id)?,
            },
        })
    }

    pub fn verified_workload(id: impl Into<String>, role: WorkloadRole) -> KernelResult<Self> {
        let actor_role = match role {
            WorkloadRole::WorkflowEngine => ActorRole::Workflow,
            WorkloadRole::RulesEngine => ActorRole::RulesEngine,
            WorkloadRole::ApiServer
            | WorkloadRole::RealtimeServer
            | WorkloadRole::AgentWorker
            | WorkloadRole::AuditWriter => ActorRole::System,
        };
        Ok(Self {
            id: EntityId::new(id)?,
            role: actor_role,
            origin: ActorOrigin::Workload { role },
        })
    }

    pub fn verified_agent_run(
        agent_id: impl Into<String>,
        run_id: impl Into<String>,
        class: AgentClass,
        campaign_id: impl Into<String>,
    ) -> KernelResult<Self> {
        Ok(Self {
            id: EntityId::new(agent_id)?,
            role: if class == AgentClass::AiKeeperOrchestrator {
                ActorRole::AiKeeper
            } else {
                ActorRole::Investigator
            },
            origin: ActorOrigin::AgentRun {
                run_id: EntityId::new(run_id)?,
                class,
                campaign_id: EntityId::new(campaign_id)?,
            },
        })
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn role(&self) -> &ActorRole {
        &self.role
    }

    pub fn origin(&self) -> &ActorOrigin {
        &self.origin
    }

    /// Canonical actor role derived from the authenticated principal. It must
    /// not be replaced with the role of a separate policy approver.
    pub fn canonical_role_name(&self) -> &'static str {
        actor_role_integrity_name(&self.role)
    }

    /// Lossless canonical origin of the authenticated principal.
    pub fn canonical_origin_wire(&self) -> EventActorOriginWire {
        event_actor_origin_wire(&self.origin)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceRef {
    campaign_id: EntityId,
    resource_type: EntityId,
    resource_id: EntityId,
}

impl ResourceRef {
    pub fn new(
        campaign_id: impl Into<String>,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
    ) -> KernelResult<Self> {
        Ok(Self {
            campaign_id: EntityId::new(campaign_id)?,
            resource_type: EntityId::new(resource_type)?,
            resource_id: EntityId::new(resource_id)?,
        })
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn resource_type(&self) -> &EntityId {
        &self.resource_type
    }

    pub fn resource_id(&self) -> &EntityId {
        &self.resource_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityBinding {
    contract_id: EntityId,
    authority_owner: EntityId,
    authority_mode: AuthorityMode,
    contract_version: u64,
}

impl AuthorityBinding {
    pub fn new(
        contract_id: impl Into<String>,
        authority_owner: impl Into<String>,
        authority_mode: AuthorityMode,
        contract_version: u64,
    ) -> KernelResult<Self> {
        if contract_version == 0 {
            return Err(TrpgError::AuthorityContractVersionConflict);
        }
        Ok(Self {
            contract_id: EntityId::new(contract_id)?,
            authority_owner: EntityId::new(authority_owner)?,
            authority_mode,
            contract_version,
        })
    }

    pub fn contract_id(&self) -> &EntityId {
        &self.contract_id
    }

    pub fn authority_owner(&self) -> &EntityId {
        &self.authority_owner
    }

    pub fn authority_mode(&self) -> &AuthorityMode {
        &self.authority_mode
    }

    pub const fn contract_version(&self) -> u64 {
        self.contract_version
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedCommandContext {
    actor: Actor,
    resource: ResourceRef,
    authority: AuthorityBinding,
    trace_id: EntityId,
    authenticated_at_unix_ms: u64,
    authentication_expires_at_unix_ms: u64,
}

impl AuthenticatedCommandContext {
    pub fn new(
        actor: Actor,
        resource: ResourceRef,
        authority: AuthorityBinding,
        trace_id: impl Into<String>,
        authenticated_at_unix_ms: u64,
        authentication_expires_at_unix_ms: u64,
    ) -> KernelResult<Self> {
        if authenticated_at_unix_ms == 0
            || authentication_expires_at_unix_ms <= authenticated_at_unix_ms
        {
            return Err(TrpgError::AuthenticationRequired);
        }
        if let ActorOrigin::AgentRun { campaign_id, .. } = actor.origin() {
            if campaign_id != resource.campaign_id() {
                return Err(TrpgError::CampaignScopeMismatch);
            }
        }
        Ok(Self {
            actor,
            resource,
            authority,
            trace_id: EntityId::new(trace_id)?,
            authenticated_at_unix_ms,
            authentication_expires_at_unix_ms,
        })
    }

    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    pub fn resource(&self) -> &ResourceRef {
        &self.resource
    }

    pub fn authority(&self) -> &AuthorityBinding {
        &self.authority
    }

    pub fn trace_id(&self) -> &EntityId {
        &self.trace_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum FormalWritePath {
    WorkflowDecision,
    RulesDecision,
    ToolDecision,
    DirectAgent,
    DirectBusiness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePolicy {
    ForkOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityVersionSnapshot {
    ruleset_version: EntityId,
    house_rules_version: EntityId,
    scenario_version: EntityId,
    prompt_version: EntityId,
    agent_pack_version: EntityId,
    tool_schema_version: EntityId,
    safety_profile_version: EntityId,
    ai_provider_snapshot: EntityId,
    model_route_snapshot: EntityId,
    character_sheet_template_version: EntityId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityVersionSnapshotDraft {
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
