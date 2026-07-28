
impl RuntimeDecision {
    pub fn new(
        decision_id: impl Into<String>,
        decision_summary: impl Into<String>,
        tool_request: ToolRequest,
    ) -> KernelResult<Self> {
        Ok(Self {
            decision_id: EntityId::new(decision_id)?,
            decision_summary: decision_summary.into(),
            tool_request,
            linked_records: vec!["DecisionRecord", "DiceRoll", "GameEvent"],
            player_visible_explanation: "Ruling resolved through the runtime decision pipeline."
                .to_owned(),
            audit_fields: vec![
                "agent_pack_version",
                "prompt_version",
                "model_provider",
                "context_hash",
                "tool_calls",
                "decision_summary",
            ],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum PendingDecisionStatus {
    DraftOnly,
    AwaitingHumanConfirmation,
    ReadyToCommit,
    Committed,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingDecision {
    pub decision: RuntimeDecision,
    pub status: PendingDecisionStatus,
    pub grant: ToolGrantDecision,
    governed: Option<GovernedPendingBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GovernedPendingBinding {
    confirmation_id: [u8; 32],
    campaign_id: EntityId,
    authority_contract_id: EntityId,
    authority_contract_version: u64,
    authority_owner: EntityId,
    draft_hash: String,
    expires_at_unix_ms: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConfirmedPendingDecision {
    pending: PendingDecision,
    confirmed_by: Actor,
    confirmation_authentication: AuthenticationContext,
    confirmed_at_unix_ms: u64,
    committed: bool,
}

impl ConfirmedPendingDecision {
    pub fn status(&self) -> PendingDecisionStatus {
        self.pending.status
    }

    pub fn confirmed_by(&self) -> &Actor {
        &self.confirmed_by
    }

    pub const fn confirmed_at_unix_ms(&self) -> u64 {
        self.confirmed_at_unix_ms
    }

    pub const fn is_committed(&self) -> bool {
        self.committed
    }
}

#[derive(Clone)]
pub struct HumanConfirmationGate {
    identity_verifier: IdentityVerifier,
    confirmation_state: Arc<Mutex<HumanConfirmationState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmationLifecycle {
    Awaiting,
    Confirmed,
    Committed,
}

#[derive(Debug)]
struct HumanConfirmationState {
    instance_nonce: [u8; 32],
    next_sequence: u64,
    lifecycle_by_id: HashMap<[u8; 32], ConfirmationLifecycle>,
}

impl std::fmt::Debug for HumanConfirmationGate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HumanConfirmationGate")
            .field("identity_verifier", &self.identity_verifier)
            .field("confirmation_state", &"[REDACTED]")
            .finish()
    }
}

impl HumanConfirmationGate {
    pub fn new(identity_verifier: IdentityVerifier) -> RuntimeResult<Self> {
        let mut instance_nonce = [0_u8; 32];
        OsRng.try_fill_bytes(&mut instance_nonce).map_err(|_| {
            RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_nonce_unavailable",
            ))
        })?;
        Ok(Self {
            identity_verifier,
            confirmation_state: Arc::new(Mutex::new(HumanConfirmationState {
                instance_nonce,
                next_sequence: 0,
                lifecycle_by_id: HashMap::new(),
            })),
        })
    }

    pub fn create_pending(
        &self,
        command: &CommandEnvelope<RuntimeDecision>,
        created_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> RuntimeResult<PendingDecision> {
        let campaign_id = command.authenticated_context().resource().campaign_id();
        let contract = canonical_contract(&self.identity_verifier, campaign_id)?;
        contract.validate_command(command)?;
        let draft_hash = canonical_commit_draft_hash(command);
        let mut state = self.confirmation_state.lock().map_err(|_| {
            RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_state_unavailable",
            ))
        })?;
        state.next_sequence = state
            .next_sequence
            .checked_add(1)
            .ok_or(RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_sequence_exhausted",
            )))?;
        let confirmation_id = confirmation_id(
            &state.instance_nonce,
            state.next_sequence,
            &contract,
            &draft_hash,
            created_at_unix_ms,
            expires_at_unix_ms,
        );
        let pending = create_governed_pending_decision(
            &contract,
            command,
            created_at_unix_ms,
            expires_at_unix_ms,
            confirmation_id,
        )?;
        state
            .lifecycle_by_id
            .insert(confirmation_id, ConfirmationLifecycle::Awaiting);
        Ok(pending)
    }

    pub fn confirm(
        &self,
        pending: &PendingDecision,
        authentication: &AuthenticationContext,
        submitted_command: &CommandEnvelope<RuntimeDecision>,
        now_unix_ms: u64,
    ) -> RuntimeResult<ConfirmedPendingDecision> {
        let binding = pending
            .governed
            .as_ref()
            .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?;
        let campaign_id = binding.campaign_id.clone();
        let confirmation_id = binding.confirmation_id;
        let contract = canonical_contract(&self.identity_verifier, &campaign_id)?;
        let confirmed = confirm_pending_decision(
            pending,
            &contract,
            &self.identity_verifier,
            authentication,
            submitted_command,
            now_unix_ms,
        )?;
        let mut state = self.confirmation_state.lock().map_err(|_| {
            RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_state_unavailable",
            ))
        })?;
        let lifecycle = state
            .lifecycle_by_id
            .get_mut(&confirmation_id)
            .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?;
        if lifecycle != &ConfirmationLifecycle::Awaiting {
            return Err(RuntimeError::Core(TrpgError::DecisionAlreadyCommitted));
        }
        *lifecycle = ConfirmationLifecycle::Confirmed;
        Ok(confirmed)
    }

    pub fn commit(
        &self,
        store: &mut EventStore<RuntimeEventPayload>,
        command: &CommandEnvelope<RuntimeDecision>,
        workflow_authentication: &AuthenticationContext,
        confirmed: &mut ConfirmedPendingDecision,
        submitted_decision: RuntimeDecision,
        now_unix_ms: u64,
    ) -> RuntimeResult<Vec<EventEnvelope<RuntimeEventPayload>>> {
        let contract = canonical_contract(
            &self.identity_verifier,
            command.authenticated_context().resource().campaign_id(),
        )?;
        contract.validate_command(command)?;
        self.identity_verifier
            .verify_actor(
                workflow_authentication,
                &command.actor,
                command.authenticated_context().resource().campaign_id(),
                now_unix_ms,
            )
            .map_err(|_| RuntimeError::Core(TrpgError::InternalIdentityInvalid))?;
        self.identity_verifier
            .verify(&confirmed.confirmation_authentication, now_unix_ms)
            .map_err(|_| RuntimeError::Core(TrpgError::InternalIdentityInvalid))?;
        let confirmation_id = confirmed
            .pending
            .governed
            .as_ref()
            .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?
            .confirmation_id;
        let mut state = self.confirmation_state.lock().map_err(|_| {
            RuntimeError::Core(TrpgError::InvalidConfiguration(
                "confirmation_state_unavailable",
            ))
        })?;
        let lifecycle = state
            .lifecycle_by_id
            .get_mut(&confirmation_id)
            .ok_or(RuntimeError::Core(TrpgError::DecisionConfirmationRequired))?;
        match lifecycle {
            ConfirmationLifecycle::Awaiting => {
                return Err(RuntimeError::Core(TrpgError::DecisionConfirmationRequired));
            }
            // A committed confirmation may reach the canonical Event Store
            // again. Its exact request hash returns the original durable
            // events; any changed draft or command is still rejected below.
            ConfirmationLifecycle::Committed | ConfirmationLifecycle::Confirmed => {}
        }
        let events = commit_confirmed_decision(
            store,
            &contract,
            command,
            workflow_authentication,
            confirmed,
            submitted_decision,
            now_unix_ms,
        )?;
        *lifecycle = ConfirmationLifecycle::Committed;
        Ok(events)
    }
}

fn canonical_contract(
    identity_verifier: &IdentityVerifier,
    campaign_id: &EntityId,
) -> RuntimeResult<AuthorityContract> {
    identity_verifier
        .authority_contract(campaign_id)
        .map_err(|_| RuntimeError::Core(TrpgError::AuthorityViolation))
}

pub fn create_pending_decision(
    authority_mode: &AuthorityMode,
    decision: RuntimeDecision,
) -> PendingDecision {
    let grant = evaluate_tool_grant(authority_mode, &decision.tool_request);
    let status = if grant.draft_only {
        PendingDecisionStatus::DraftOnly
    } else if grant.requires_human_confirmation {
        PendingDecisionStatus::AwaitingHumanConfirmation
    } else {
        PendingDecisionStatus::ReadyToCommit
    };

    PendingDecision {
        decision,
        status,
        grant,
        governed: None,
    }
}

fn create_governed_pending_decision(
    contract: &AuthorityContract,
    command: &CommandEnvelope<RuntimeDecision>,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    confirmation_id: [u8; 32],
) -> RuntimeResult<PendingDecision> {
    if created_at_unix_ms == 0 || expires_at_unix_ms <= created_at_unix_ms {
        return Err(RuntimeError::Core(TrpgError::DecisionExpired));
    }
    let mut pending = create_pending_decision(contract.mode(), command.payload.clone());
    if contract.mode() == &AuthorityMode::HumanKp
        && pending.decision.tool_request.is_formal_state_change()
    {
        pending.status = PendingDecisionStatus::AwaitingHumanConfirmation;
    }
    pending.governed = Some(GovernedPendingBinding {
        confirmation_id,
        campaign_id: contract.campaign_id().clone(),
        authority_contract_id: contract.contract_id().clone(),
        authority_contract_version: contract.version(),
        authority_owner: contract.authority_owner().clone(),
        draft_hash: canonical_commit_draft_hash(command),
        expires_at_unix_ms,
    });
    Ok(pending)
}

fn confirmation_id(
    instance_nonce: &[u8; 32],
    sequence: u64,
    contract: &AuthorityContract,
    draft_hash: &str,
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(instance_nonce);
    hasher.update(sequence.to_be_bytes());
    hasher.update(contract.campaign_id().as_str().as_bytes());
    hasher.update(contract.contract_id().as_str().as_bytes());
    hasher.update(contract.version().to_be_bytes());
    hasher.update(draft_hash.as_bytes());
    hasher.update(created_at_unix_ms.to_be_bytes());
    hasher.update(expires_at_unix_ms.to_be_bytes());
    hasher.finalize().into()
}
