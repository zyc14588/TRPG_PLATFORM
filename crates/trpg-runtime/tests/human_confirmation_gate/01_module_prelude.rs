use std::sync::{Arc, Barrier};
use std::thread;

use trpg_identity::{AuthenticationContext, GlobalRole, IdentityError, IdentityService};
use trpg_runtime::runtime_state_machines::{
    commit_decision, HumanConfirmationGate, PendingDecisionStatus, RuntimeAgent, RuntimeDecision,
    RuntimeError, RuntimeResult, RuntimeTool, ToolRequest,
};
use trpg_runtime::{
    ActorRole, AuthorityMode, EventStore, FormalCommitAudit, FormalCommitAuthorizer,
    RuntimeToolExecutionOutput, RuntimeToolExecutor, TrpgError,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_shared_kernel::AuthorityContract;

fn contract() -> AuthorityContract {
    trpg_test_support::authority_contract_with_owner(
        "campaign_human",
        AuthorityMode::HumanKp,
        "keeper_owner",
        3,
    )
    .unwrap()
}

fn decision() -> RuntimeDecision {
    RuntimeDecision::new(
        "decision_human_001",
        "Keeper copilot proposes a formal ruling",
        ToolRequest::formal(RuntimeAgent::KeeperCopilot, RuntimeTool::CommitDecision),
    )
    .unwrap()
}

struct SuccessfulRuntimeToolExecutor;

impl RuntimeToolExecutor for SuccessfulRuntimeToolExecutor {
    fn execute(&self, decision: &RuntimeDecision) -> RuntimeResult<RuntimeToolExecutionOutput> {
        Ok(RuntimeToolExecutionOutput {
            execution_id: format!("execution_{}", decision.decision_id.as_str()),
            result_hash: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                .to_owned(),
        })
    }
}

fn authentication(identity: &mut IdentityService, subject: &str) -> AuthenticationContext {
    let login = format!("{subject}@example.test");
    let password = match identity.create_user(
        subject,
        &login,
        "confirmation password long enough",
        GlobalRole::User,
    ) {
        Ok(()) => "confirmation password long enough",
        Err(IdentityError::DuplicateLogin) => "test authority password long enough",
        Err(error) => panic!("failed to create confirmation identity: {error}"),
    };
    let session = identity.login(&login, password, 100).unwrap();
    identity
        .authenticate_session(Some(session.token.expose()), 101)
        .unwrap()
}

fn rogue_authentication(subject: &str) -> AuthenticationContext {
    let mut identity = IdentityService::new(&[0x99; 32], 60_000).unwrap();
    authentication(&mut identity, subject)
}

fn rogue_workflow_authentication() -> AuthenticationContext {
    let identity = IdentityService::new(&[0x99; 32], 60_000).unwrap();
    let credential = identity
        .issue_workload_credential(
            "workflow_001",
            trpg_identity::WorkloadRole::WorkflowEngine,
            1,
            10_000,
        )
        .unwrap();
    identity.authenticate_workload(&credential, 2).unwrap()
}

fn gate(contract: &AuthorityContract) -> (HumanConfirmationGate, IdentityService) {
    let identity = trpg_test_support::identity_service_for_contract(contract);
    let gate = HumanConfirmationGate::new(identity.verifier()).unwrap();
    (gate, identity)
}

fn formal_audit(name: &str) -> FormalCommitAudit {
    let path = std::env::temp_dir().join(format!(
        "p02-runtime-formal-audit-{}-{name}.jsonl",
        std::process::id()
    ));
    let mut anchor_name = path.as_os_str().to_os_string();
    anchor_name.push(".head");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(std::path::PathBuf::from(anchor_name));
    FormalCommitAudit::open(path, "runtime-test-key-v1", &[0x82; 32]).unwrap()
}

fn formal_authorizer(
    identity_verifier: trpg_identity::IdentityVerifier,
    audit: FormalCommitAudit,
) -> FormalCommitAuthorizer {
    let endpoints = trpg_test_support::formal_commit_policy_endpoints();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            endpoints.openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            endpoints.openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            endpoints.opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            endpoints.opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    FormalCommitAuthorizer::new(identity_verifier, policy, audit)
}

#[test]
fn unconfirmed_non_owner_changed_and_expired_drafts_are_rejected() {
    let contract = contract();
    let (gate, mut identity) = gate(&contract);
    let decision = decision();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = EventStore::default();

    assert_eq!(
        commit_decision(
            &mut store,
            &contract,
            &command,
            &trpg_test_support::workflow_authentication(),
            decision.clone(),
            160,
        )
        .unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionConfirmationRequired)
    );
    assert!(store.events().is_empty());

    let pending = gate.create_pending(&command, 100, 200).unwrap();
    assert_eq!(
        pending.status,
        PendingDecisionStatus::AwaitingHumanConfirmation
    );
    let attacker = authentication(&mut identity, "keeper_attacker");
    assert_eq!(
        gate.confirm(&pending, &attacker, &command, 150)
            .unwrap_err(),
        RuntimeError::Core(TrpgError::AuthorityOwnerMismatch)
    );

    let owner = authentication(&mut identity, "keeper_owner");
    let mut changed = decision.clone();
    changed.decision_summary.push_str(" after confirmation");
    let mut changed_command = command.clone();
    changed_command.payload = changed;
    assert_eq!(
        gate.confirm(&pending, &owner, &changed_command, 150)
            .unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionDraftChanged)
    );
    assert_eq!(
        gate.confirm(&pending, &owner, &command, 201).unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionExpired)
    );
}

#[test]
fn owner_confirmation_commits_exact_draft_once() {
    let contract = contract();
    let (gate, mut identity) = gate(&contract);
    let decision = decision();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let pending = gate.create_pending(&command, 100, 200).unwrap();
    let owner = authentication(&mut identity, "keeper_owner");
    let mut confirmed = gate.confirm(&pending, &owner, &command, 150).unwrap();
    assert_eq!(confirmed.status(), PendingDecisionStatus::ReadyToCommit);
    assert_eq!(confirmed.confirmed_by().id().as_str(), "keeper_owner");

    let audit = formal_audit("owner-confirmation");
    let workflow_authentication = trpg_test_support::workflow_authentication();
    let mut store = EventStore::with_formal_custody_and_executor(
        formal_authorizer(identity.verifier(), audit.clone()),
        trpg_test_support::test_canonical_commit_port(),
        Arc::new(SuccessfulRuntimeToolExecutor),
    );
    assert_eq!(
        gate.commit(
            &mut store,
            &command,
            &rogue_workflow_authentication(),
            &mut confirmed,
            decision.clone(),
            160,
        )
        .unwrap_err(),
        RuntimeError::Core(TrpgError::InternalIdentityInvalid)
    );
    assert!(store.events().is_empty());
    let events = gate
        .commit(
            &mut store,
            &command,
            &workflow_authentication,
            &mut confirmed,
            decision.clone(),
            160,
        )
        .unwrap();
    assert_eq!(events.len(), 3);
    assert!(confirmed.is_committed());
    assert_eq!(confirmed.status(), PendingDecisionStatus::Committed);

    let retried = gate
        .commit(
            &mut store,
            &command,
            &trpg_test_support::workflow_authentication(),
            &mut confirmed,
            decision.clone(),
            170,
        )
        .unwrap();
    assert_eq!(retried, events);
    assert_eq!(store.events().len(), 3);

    let mut changed_retry = decision;
    changed_retry
        .decision_summary
        .push_str(" changed after durable commit");
    assert_eq!(
        gate.commit(
            &mut store,
            &command,
            &trpg_test_support::workflow_authentication(),
            &mut confirmed,
            changed_retry,
            180,
        )
        .unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionDraftChanged)
    );
    assert_eq!(store.events().len(), 3);
    let records = audit.verify().unwrap();
    // Both authenticated write attempts are auditable, while Event Store
    // idempotency keeps the canonical event batch single-copy.
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .all(|record| record.actor_id == "keeper_owner"));
    assert_eq!(records[0].actor_id, "keeper_owner");
    assert_eq!(records[0].action, "write_official_state");
    assert_eq!(records[0].requested_role, "human_keeper");
    assert_eq!(records[0].campaign_id, "campaign_human");
}

#[test]
fn human_confirmation_binds_visibility_and_fact_provenance() {
    let contract = contract();
    let (gate, mut identity) = gate(&contract);
    let decision = decision();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let pending = gate.create_pending(&command, 100, 200).unwrap();
    let owner = authentication(&mut identity, "keeper_owner");
    let mut confirmed = gate.confirm(&pending, &owner, &command, 150).unwrap();
    let mut command = command;
    command.visibility =
        trpg_shared_kernel::Visibility::new(trpg_shared_kernel::VisibilityLabel::KeeperOnly);
    command.fact_provenance = trpg_shared_kernel::FactProvenance::new(
        trpg_shared_kernel::ProvenanceKind::HumanKeeperStatement,
        "changed_after_confirmation",
        "keeper_owner",
    )
    .unwrap();
    let audit = formal_audit("security-metadata-binding");
    let mut store = EventStore::with_formal_custody(
        formal_authorizer(identity.verifier(), audit),
        trpg_test_support::test_canonical_commit_port(),
    );

    assert_eq!(
        gate.commit(
            &mut store,
            &command,
            &trpg_test_support::workflow_authentication(),
            &mut confirmed,
            decision,
            160,
        )
        .unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionDraftChanged)
    );
    assert!(store.events().is_empty());
}

#[test]
fn formal_commit_fails_closed_without_external_audit() {
    let contract = contract();
    let (gate, mut identity) = gate(&contract);
    let decision = decision();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let pending = gate.create_pending(&command, 100, 200).unwrap();
    let owner = authentication(&mut identity, "keeper_owner");
    let mut confirmed = gate.confirm(&pending, &owner, &command, 150).unwrap();
    let mut store = EventStore::default();

    assert_eq!(
        gate.commit(
            &mut store,
            &command,
            &trpg_test_support::workflow_authentication(),
            &mut confirmed,
            decision,
            160,
        )
        .unwrap_err(),
        RuntimeError::Core(TrpgError::AuditIntegrityViolation)
    );
    assert!(store.events().is_empty());
    assert!(!confirmed.is_committed());
}
