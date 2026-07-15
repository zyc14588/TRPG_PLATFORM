mod common;

use trpg_agent_runtime::agent_runtime::{AgentDecision, AgentDecisionCommitter};
use trpg_agent_runtime::{
    ActorRole, AgentError, AgentKind, AgentTool, AuthorityMode, ToolRequest, TrpgError,
};
use trpg_identity::{AgentClass, IdentityService};

const TRUSTED_KEY: [u8; 32] = [0x5a; 32];

fn committer_for(contract: trpg_agent_runtime::AuthorityContract) -> AgentDecisionCommitter {
    AgentDecisionCommitter::new(trpg_test_support::identity_verifier_for_contract(&contract))
        .unwrap()
}

#[test]
fn caller_reported_agent_kind_cannot_replace_verified_agent_run() {
    let authentication = trpg_test_support::ai_keeper_authentication("campaign_a");
    let forged_request =
        ToolRequest::formal(AgentKind::KeeperCopilot, AgentTool::RequestSkillCheck);
    assert_eq!(
        AgentDecision::new(
            "decision_forged_agent",
            forged_request,
            "forged",
            &authentication,
        )
        .unwrap_err(),
        AgentError::Core(TrpgError::InternalIdentityInvalid)
    );
}

#[test]
fn verified_agent_run_is_scoped_to_its_campaign() {
    let authentication = trpg_test_support::ai_keeper_authentication("campaign_a");
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let decision = AgentDecision::new(
        "decision_cross_campaign_agent",
        request,
        "forged cross campaign",
        &authentication,
    )
    .unwrap();
    let contract =
        trpg_test_support::authority_contract("campaign_b", AuthorityMode::AiKp, 1).unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = common::audited_store(&contract);
    let committer = committer_for(contract);

    assert_eq!(
        committer
            .commit(
                &mut store,
                &command,
                &trpg_test_support::workflow_authentication(),
                decision,
                2,
            )
            .unwrap_err(),
        AgentError::Core(TrpgError::CampaignScopeMismatch)
    );
    assert!(store.events().is_empty());
}

#[test]
fn rogue_identity_issuer_cannot_authorize_a_formal_agent_commit() {
    let contract =
        trpg_test_support::authority_contract("campaign_a", AuthorityMode::AiKp, 1).unwrap();
    let rogue_identity = IdentityService::new(&[0x77; 32], 60_000).unwrap();
    let credential = rogue_identity
        .issue_agent_run_credential(
            "rogue_run",
            contract.authority_owner().as_str(),
            contract.campaign_id().as_str(),
            AgentClass::AiKeeperOrchestrator,
            1,
            10_000,
        )
        .unwrap();
    let authentication = rogue_identity
        .authenticate_agent_run(&credential, 2)
        .unwrap();
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let decision = AgentDecision::new(
        "decision_rogue_issuer",
        request,
        "forged issuer",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = common::audited_store(&contract);
    let committer = committer_for(contract);

    assert_eq!(
        committer
            .commit(
                &mut store,
                &command,
                &trpg_test_support::workflow_authentication(),
                decision,
                2,
            )
            .unwrap_err(),
        AgentError::Core(TrpgError::InternalIdentityInvalid)
    );
    assert!(store.events().is_empty());
}

#[test]
fn trusted_but_non_owner_ai_cannot_commit_in_ai_kp_mode() {
    let contract =
        trpg_test_support::authority_contract("campaign_a", AuthorityMode::AiKp, 1).unwrap();
    let trusted_identity = IdentityService::new(&TRUSTED_KEY, 60_000).unwrap();
    let credential = trusted_identity
        .issue_agent_run_credential(
            "non_owner_run",
            "ai_kp_not_the_owner",
            contract.campaign_id().as_str(),
            AgentClass::AiKeeperOrchestrator,
            1,
            10_000,
        )
        .unwrap();
    let authentication = trusted_identity
        .authenticate_agent_run(&credential, 2)
        .unwrap();
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let decision = AgentDecision::new(
        "decision_non_owner",
        request,
        "wrong owner",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = common::audited_store(&contract);
    let committer = committer_for(contract);

    assert_eq!(
        committer
            .commit(
                &mut store,
                &command,
                &trpg_test_support::workflow_authentication(),
                decision,
                2,
            )
            .unwrap_err(),
        AgentError::Core(TrpgError::AuthorityOwnerMismatch)
    );
    assert!(store.events().is_empty());
}

#[test]
fn caller_supplied_contract_cannot_replace_the_identity_roots_canonical_contract() {
    let canonical = trpg_test_support::authority_contract_with_owner(
        "campaign_a",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let conflict = trpg_test_support::authority_contract_with_owner(
        "campaign_a",
        AuthorityMode::AiKp,
        "ai_kp_other",
        1,
    )
    .unwrap();
    let committer = AgentDecisionCommitter::new(trpg_test_support::identity_verifier_for_contract(
        &canonical,
    ))
    .unwrap();
    let authentication = trpg_test_support::ai_keeper_authentication("campaign_a");
    let decision = AgentDecision::new(
        "decision_conflicting_authority",
        ToolRequest::formal(
            AgentKind::AiKeeperOrchestrator,
            AgentTool::RequestSkillCheck,
        ),
        "conflicting authority",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &conflict,
        decision.clone(),
        ActorRole::Workflow,
    );

    assert_eq!(
        committer
            .commit(
                &mut common::audited_store(&canonical),
                &command,
                &trpg_test_support::workflow_authentication(),
                decision,
                2,
            )
            .unwrap_err(),
        AgentError::Core(TrpgError::AuthorityOwnerMismatch)
    );
}

#[test]
fn draft_constructor_cannot_disguise_an_adjudicating_tool() {
    let request = ToolRequest::draft(AgentKind::AiKeeperOrchestrator, AgentTool::ApplySanLoss);

    assert_eq!(request.tool(), AgentTool::DraftSanLoss);
    assert!(!request.is_formal_state_change());
}

#[test]
fn explicit_draft_never_emits_a_formal_decision_committed_event() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_human",
        AuthorityMode::HumanKp,
        "human_keeper",
        1,
    )
    .unwrap();
    let identity = IdentityService::new(&TRUSTED_KEY, 60_000).unwrap();
    let credential = identity
        .issue_agent_run_credential(
            "copilot_run",
            "keeper_copilot",
            "campaign_human",
            AgentClass::KeeperCopilot,
            1,
            10_000,
        )
        .unwrap();
    let authentication = identity.authenticate_agent_run(&credential, 2).unwrap();
    let request = ToolRequest::draft(AgentKind::KeeperCopilot, AgentTool::ApplySanLoss);
    let decision = AgentDecision::new(
        "decision_explicit_draft",
        request,
        "draft only",
        &authentication,
    )
    .unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    let mut store = common::audited_store(&contract);
    let committer = committer_for(contract);

    let events = committer
        .commit(
            &mut store,
            &command,
            &trpg_test_support::workflow_authentication(),
            decision,
            2,
        )
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "DraftDecisionCreated");
    assert!(store
        .events()
        .iter()
        .all(|event| event.event_type != "DecisionCommitted"));
}
