use std::sync::{Arc, Barrier};
use std::thread;

use trpg_identity::{AuthenticationContext, GlobalRole, IdentityService, IdentityVerifier};
use trpg_runtime::runtime_state_machines::{
    commit_decision, HumanConfirmationGate, PendingDecisionStatus, RuntimeAgent, RuntimeDecision,
    RuntimeError, RuntimeTool, ToolRequest,
};
use trpg_runtime::{ActorRole, AuthorityMode, EventStore, TrpgError};
use trpg_shared_kernel::AuthorityContract;

const TRUSTED_KEY: [u8; 32] = [0x45; 32];

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

fn authentication(subject: &str, key: &[u8; 32]) -> (IdentityVerifier, AuthenticationContext) {
    let mut identity = IdentityService::new(key, 60_000).unwrap();
    let login = format!("{subject}@example.test");
    identity
        .create_user(
            subject,
            &login,
            "confirmation password long enough",
            GlobalRole::User,
        )
        .unwrap();
    let session = identity
        .login(&login, "confirmation password long enough", 100)
        .unwrap();
    let context = identity
        .authenticate_session(Some(session.token.expose()), 101)
        .unwrap();
    (identity.verifier(), context)
}

fn gate(contract: &AuthorityContract) -> HumanConfirmationGate {
    let identity = IdentityService::new(&TRUSTED_KEY, 60_000).unwrap();
    HumanConfirmationGate::new(identity.verifier(), [contract.clone()]).unwrap()
}

#[test]
fn unconfirmed_non_owner_changed_and_expired_drafts_are_rejected() {
    let contract = contract();
    let gate = gate(&contract);
    let decision = decision();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = EventStore::default();

    assert_eq!(
        commit_decision(&mut store, &contract, &command, decision.clone()).unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionConfirmationRequired)
    );
    assert!(store.events().is_empty());

    let pending = gate
        .create_pending(contract.campaign_id(), decision.clone(), 100, 200)
        .unwrap();
    assert_eq!(
        pending.status,
        PendingDecisionStatus::AwaitingHumanConfirmation
    );
    let (_, attacker) = authentication("keeper_attacker", &TRUSTED_KEY);
    assert_eq!(
        gate.confirm(&pending, &attacker, &decision, 150)
            .unwrap_err(),
        RuntimeError::Core(TrpgError::AuthorityOwnerMismatch)
    );

    let (_, owner) = authentication("keeper_owner", &TRUSTED_KEY);
    let mut changed = decision.clone();
    changed.decision_summary.push_str(" after confirmation");
    assert_eq!(
        gate.confirm(&pending, &owner, &changed, 150).unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionDraftChanged)
    );
    assert_eq!(
        gate.confirm(&pending, &owner, &decision, 201).unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionExpired)
    );
}

#[test]
fn owner_confirmation_commits_exact_draft_once() {
    let contract = contract();
    let gate = gate(&contract);
    let decision = decision();
    let pending = gate
        .create_pending(contract.campaign_id(), decision.clone(), 100, 200)
        .unwrap();
    let (_, owner) = authentication("keeper_owner", &TRUSTED_KEY);
    let mut confirmed = gate.confirm(&pending, &owner, &decision, 150).unwrap();
    assert_eq!(confirmed.status(), PendingDecisionStatus::ReadyToCommit);
    assert_eq!(confirmed.confirmed_by().id().as_str(), "keeper_owner");

    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = EventStore::default();
    let events = gate
        .commit(&mut store, &command, &mut confirmed, decision.clone(), 160)
        .unwrap();
    assert_eq!(events.len(), 2);
    assert!(confirmed.is_committed());
    assert_eq!(confirmed.status(), PendingDecisionStatus::Committed);

    assert_eq!(
        gate.commit(&mut store, &command, &mut confirmed, decision, 170)
            .unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionAlreadyCommitted)
    );
    assert_eq!(store.events().len(), 2);
}

#[test]
fn one_pending_issues_one_confirmation_even_concurrently_and_not_after_restart() {
    let contract = contract();
    let confirmation_gate = gate(&contract);
    let decision = decision();
    let pending = confirmation_gate
        .create_pending(contract.campaign_id(), decision.clone(), 100, 200)
        .unwrap();
    let (_, owner) = authentication("keeper_owner", &TRUSTED_KEY);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let worker_gate = confirmation_gate.clone();
        let worker_pending = pending.clone();
        let worker_owner = owner.clone();
        let worker_decision = decision.clone();
        let worker_barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            worker_barrier.wait();
            worker_gate.confirm(&worker_pending, &worker_owner, &worker_decision, 150)
        }));
    }
    barrier.wait();

    let mut issued = 0;
    let mut rejected = 0;
    for result in workers.into_iter().map(|worker| worker.join().unwrap()) {
        match result {
            Ok(_) => issued += 1,
            Err(RuntimeError::Core(TrpgError::DecisionAlreadyCommitted)) => rejected += 1,
            Err(error) => panic!("unexpected confirmation result: {error:?}"),
        }
    }
    assert_eq!(issued, 1);
    assert_eq!(rejected, 1);

    let restarted_gate = gate(&contract);
    assert_eq!(
        restarted_gate
            .confirm(&pending, &owner, &decision, 150)
            .unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionConfirmationRequired)
    );
}

#[test]
fn same_subject_from_a_rogue_identity_issuer_cannot_confirm() {
    let contract = contract();
    let gate = gate(&contract);
    let decision = decision();
    let pending = gate
        .create_pending(contract.campaign_id(), decision.clone(), 100, 200)
        .unwrap();
    let (_, rogue_owner) = authentication("keeper_owner", &[0x99; 32]);

    assert_eq!(
        gate.confirm(&pending, &rogue_owner, &decision, 150)
            .unwrap_err(),
        RuntimeError::Core(TrpgError::InternalIdentityInvalid)
    );
}

#[test]
fn conflicting_authority_contracts_cannot_initialize_the_gate() {
    let canonical = contract();
    let conflicting = trpg_test_support::authority_contract_with_owner(
        canonical.campaign_id().as_str(),
        AuthorityMode::HumanKp,
        "different_keeper",
        canonical.version(),
    )
    .unwrap();
    let identity = IdentityService::new(&TRUSTED_KEY, 60_000).unwrap();

    assert_eq!(
        HumanConfirmationGate::new(identity.verifier(), [canonical, conflicting]).unwrap_err(),
        RuntimeError::Core(TrpgError::AuthorityContractMutation)
    );
}

#[test]
fn draft_label_cannot_disguise_or_commit_an_adjudicating_tool() {
    let contract = contract();
    let request = ToolRequest::draft(RuntimeAgent::HumanKeeper, RuntimeTool::CommitDecision);
    assert_eq!(request.tool(), RuntimeTool::NarrationOnly);
    assert!(!request.is_formal_state_change());
    let decision = RuntimeDecision::new("decision_draft_disguise", "draft", request).unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = EventStore::default();

    assert_eq!(
        commit_decision(&mut store, &contract, &command, decision).unwrap_err(),
        RuntimeError::Core(TrpgError::PolicyDenied)
    );
    assert!(store.events().is_empty());
}
