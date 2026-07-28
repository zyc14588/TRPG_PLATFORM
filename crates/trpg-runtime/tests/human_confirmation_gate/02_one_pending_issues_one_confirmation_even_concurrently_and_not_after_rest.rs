
#[test]
fn one_pending_issues_one_confirmation_even_concurrently_and_not_after_restart() {
    let contract = contract();
    let (confirmation_gate, mut identity) = gate(&contract);
    let decision = decision();
    let command =
        trpg_test_support::governed_command_for_contract(&contract, decision, ActorRole::Workflow);
    let pending = confirmation_gate
        .create_pending(&command, 100, 200)
        .unwrap();
    let owner = authentication(&mut identity, "keeper_owner");
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let worker_gate = confirmation_gate.clone();
        let worker_pending = pending.clone();
        let worker_owner = owner.clone();
        let worker_command = command.clone();
        let worker_barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            worker_barrier.wait();
            worker_gate.confirm(&worker_pending, &worker_owner, &worker_command, 150)
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

    let restarted_gate = HumanConfirmationGate::new(identity.verifier()).unwrap();
    assert_eq!(
        restarted_gate
            .confirm(&pending, &owner, &command, 150)
            .unwrap_err(),
        RuntimeError::Core(TrpgError::DecisionConfirmationRequired)
    );
}

#[test]
fn same_subject_from_a_rogue_identity_issuer_cannot_confirm() {
    let contract = contract();
    let (gate, _identity) = gate(&contract);
    let decision = decision();
    let command =
        trpg_test_support::governed_command_for_contract(&contract, decision, ActorRole::Workflow);
    let pending = gate.create_pending(&command, 100, 200).unwrap();
    let rogue_owner = rogue_authentication("keeper_owner");

    assert_eq!(
        gate.confirm(&pending, &rogue_owner, &command, 150)
            .unwrap_err(),
        RuntimeError::Core(TrpgError::InternalIdentityInvalid)
    );
}

#[test]
fn caller_contract_cannot_replace_the_identity_roots_canonical_contract() {
    let canonical = contract();
    let conflicting = trpg_test_support::authority_contract_with_owner(
        canonical.campaign_id().as_str(),
        AuthorityMode::HumanKp,
        "different_keeper",
        canonical.version(),
    )
    .unwrap();
    let (gate, mut identity) = gate(&canonical);
    let decision = decision();
    let canonical_command = trpg_test_support::governed_command_for_contract(
        &canonical,
        decision.clone(),
        ActorRole::Workflow,
    );
    let pending = gate.create_pending(&canonical_command, 100, 200).unwrap();
    let owner = authentication(&mut identity, "keeper_owner");
    let mut confirmed = gate
        .confirm(&pending, &owner, &canonical_command, 150)
        .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &conflicting,
        decision.clone(),
        ActorRole::Workflow,
    );

    assert_eq!(
        gate.commit(
            &mut EventStore::default(),
            &command,
            &trpg_test_support::workflow_authentication(),
            &mut confirmed,
            decision,
            160,
        )
        .unwrap_err(),
        RuntimeError::Core(TrpgError::AuthorityOwnerMismatch)
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
        commit_decision(
            &mut store,
            &contract,
            &command,
            &trpg_test_support::workflow_authentication(),
            decision,
            160,
        )
        .unwrap_err(),
        RuntimeError::Core(TrpgError::PolicyDenied)
    );
    assert!(store.events().is_empty());
}
