
impl<P: Clone> EventStore<P> {
    /// Compatibility replay for single-campaign in-memory stores. A store
    /// containing more than one campaign fails closed; callers that own an
    /// authenticated campaign scope must use `replay_visible_in_campaign`.
    pub fn replay_visible(&self, principal: &PrincipalScope) -> Vec<EventEnvelope<P>> {
        let Some(campaign_id) = self.events.first().map(|event| &event.campaign_id) else {
            return Vec::new();
        };
        if self
            .events
            .iter()
            .any(|event| &event.campaign_id != campaign_id)
        {
            return Vec::new();
        }
        self.replay_visible_in_campaign(campaign_id, principal)
    }

    pub fn replay_visible_in_campaign(
        &self,
        campaign_id: &EntityId,
        principal: &PrincipalScope,
    ) -> Vec<EventEnvelope<P>> {
        self.events
            .iter()
            .filter(|event| {
                &event.campaign_id == campaign_id && event.visibility.can_view(principal)
            })
            .cloned()
            .collect()
    }
}

fn event_integrity_hash<P: Serialize>(event: &EventEnvelope<P>) -> KernelResult<[u8; 32]> {
    let mut digest = Sha256::new();
    hash_integrity_field(&mut digest, 1, b"trpg-event-integrity-v4");
    hash_integrity_field(&mut digest, 2, &event.sequence.to_be_bytes());
    hash_integrity_field(&mut digest, 3, event.event_type.as_bytes());
    hash_integrity_field(&mut digest, 4, event.campaign_id.as_str().as_bytes());
    hash_integrity_field(
        &mut digest,
        5,
        event.authenticated_actor.id().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        6,
        actor_role_integrity_name(event.authenticated_actor.role()).as_bytes(),
    );
    hash_actor_origin(&mut digest, event.authenticated_actor.origin());
    hash_integrity_field(
        &mut digest,
        11,
        event.resource.campaign_id().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        12,
        event.resource.resource_type().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        13,
        event.resource.resource_id().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        14,
        event.authority_contract_id.as_str().as_bytes(),
    );
    hash_integrity_field(&mut digest, 15, event.authority_owner.as_str().as_bytes());
    hash_integrity_field(&mut digest, 16, event.command_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 17, event.idempotency_key.as_bytes());
    hash_integrity_field(
        &mut digest,
        18,
        &event.authority_contract_version.to_be_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        19,
        event.visibility.label().as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        20,
        event
            .visibility
            .subject_id()
            .map(EntityId::as_str)
            .unwrap_or_default()
            .as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        21,
        provenance_kind_integrity_name(&event.fact_provenance.kind).as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        22,
        event.fact_provenance.reference.as_str().as_bytes(),
    );
    hash_integrity_field(
        &mut digest,
        23,
        event.fact_provenance.recorded_by.as_str().as_bytes(),
    );
    hash_integrity_field(&mut digest, 24, event.correlation_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 25, event.causation_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 26, event.trace_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 27, &event.occurred_at_unix_ms.to_be_bytes());
    hash_integrity_field(&mut digest, 28, &canonical_json_bytes(&event.payload)?);
    hash_integrity_field(&mut digest, 29, event.stream_id.as_str().as_bytes());
    hash_integrity_field(&mut digest, 30, &event.stream_version.to_be_bytes());
    Ok(digest.finalize().into())
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> KernelResult<Vec<u8>> {
    let mut value = serde_json::to_value(value).map_err(|_| TrpgError::PolicyEvidenceUntrusted)?;
    canonicalize_json_value(&mut value);
    serde_json::to_vec(&value).map_err(|_| TrpgError::PolicyEvidenceUntrusted)
}

fn canonicalize_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                canonicalize_json_value(value);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                canonicalize_json_value(value);
            }
            fields.sort_keys();
        }
        _ => {}
    }
}

fn hash_integrity_field(digest: &mut Sha256, tag: u16, bytes: &[u8]) {
    digest.update(tag.to_be_bytes());
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn hash_actor_origin(digest: &mut Sha256, origin: &ActorOrigin) {
    match origin {
        ActorOrigin::UserSession { session_id } => {
            hash_integrity_field(digest, 7, b"user_session");
            hash_integrity_field(digest, 8, session_id.as_str().as_bytes());
        }
        ActorOrigin::Workload { role } => {
            hash_integrity_field(digest, 7, b"workload");
            hash_integrity_field(digest, 8, workload_role_integrity_name(*role).as_bytes());
        }
        ActorOrigin::AgentRun {
            run_id,
            class,
            campaign_id,
        } => {
            hash_integrity_field(digest, 7, b"agent_run");
            hash_integrity_field(digest, 8, run_id.as_str().as_bytes());
            hash_integrity_field(digest, 9, agent_class_integrity_name(*class).as_bytes());
            hash_integrity_field(digest, 10, campaign_id.as_str().as_bytes());
        }
    }
}

fn event_actor_origin_wire(origin: &ActorOrigin) -> EventActorOriginWire {
    match origin {
        ActorOrigin::UserSession { session_id } => EventActorOriginWire::UserSession {
            session_id: session_id.to_string(),
        },
        ActorOrigin::Workload { role } => EventActorOriginWire::Workload {
            role: workload_role_integrity_name(*role).to_owned(),
        },
        ActorOrigin::AgentRun {
            run_id,
            class,
            campaign_id,
        } => EventActorOriginWire::AgentRun {
            run_id: run_id.to_string(),
            class: agent_class_integrity_name(*class).to_owned(),
            campaign_id: campaign_id.to_string(),
        },
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn actor_role_integrity_name(role: &ActorRole) -> &'static str {
    match role {
        ActorRole::ServerOwner => "server_owner",
        ActorRole::CampaignOwner => "campaign_owner",
        ActorRole::HumanKeeper => "human_keeper",
        ActorRole::AiKeeper => "ai_keeper",
        ActorRole::Investigator => "investigator",
        ActorRole::Moderator => "moderator",
        ActorRole::Spectator => "spectator",
        ActorRole::Workflow => "workflow",
        ActorRole::RulesEngine => "rules_engine",
        ActorRole::System => "system",
    }
}

fn workload_role_integrity_name(role: WorkloadRole) -> &'static str {
    match role {
        WorkloadRole::ApiServer => "api_server",
        WorkloadRole::RealtimeServer => "realtime_server",
        WorkloadRole::AgentWorker => "agent_worker",
        WorkloadRole::WorkflowEngine => "workflow_engine",
        WorkloadRole::RulesEngine => "rules_engine",
        WorkloadRole::AuditWriter => "audit_writer",
    }
}

fn agent_class_integrity_name(class: AgentClass) -> &'static str {
    match class {
        AgentClass::AiKeeperOrchestrator => "ai_keeper_orchestrator",
        AgentClass::KeeperCopilot => "keeper_copilot",
        AgentClass::AtmosphereWriter => "atmosphere_writer",
        AgentClass::MemoryCurator => "memory_curator",
    }
}

fn provenance_kind_integrity_name(kind: &ProvenanceKind) -> &'static str {
    match kind {
        ProvenanceKind::UserStatement => "user_statement",
        ProvenanceKind::HumanKeeperStatement => "human_keeper_statement",
        ProvenanceKind::RulesEngineDecision => "rules_engine_decision",
        ProvenanceKind::ToolResult => "tool_result",
        ProvenanceKind::AgentProposal => "agent_proposal",
        ProvenanceKind::ImportedSource => "imported_source",
        ProvenanceKind::SystemFixture => "system_fixture",
    }
}

fn unix_time_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelContractSnapshot {
    pub id_format: &'static str,
    pub version_policy: &'static str,
    pub visibility_enum: Vec<&'static str>,
    pub error_codes: Vec<&'static str>,
}

pub fn kernel_contract_snapshot() -> KernelContractSnapshot {
    KernelContractSnapshot {
        id_format: "non_empty_ascii_alnum_underscore_dash",
        version_policy: "expected_version_plus_immutable_authority_contract",
        visibility_enum: vec![
            VisibilityKind::Public.as_str(),
            VisibilityKind::PartyVisible.as_str(),
            VisibilityKind::PrivateToPlayer.as_str(),
            VisibilityKind::PrivateToGroup.as_str(),
            VisibilityKind::KeeperOnly.as_str(),
            VisibilityKind::InvestigatorPrivate.as_str(),
            VisibilityKind::AiInternal.as_str(),
            VisibilityKind::SystemOnly.as_str(),
            VisibilityKind::SpectatorVisible.as_str(),
            VisibilityKind::SpectatorHidden.as_str(),
            VisibilityKind::SystemPrivate.as_str(),
        ],
        error_codes: vec![
            TrpgError::InvalidEntityId.code(),
            TrpgError::UnknownVisibilityLabel.code(),
            TrpgError::AuthorityViolation.code(),
            TrpgError::ExpectedVersionConflict {
                expected: 0,
                actual: 1,
            }
            .code(),
        ],
    }
}
