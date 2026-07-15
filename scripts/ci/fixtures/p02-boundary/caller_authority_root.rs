use trpg_agent_runtime::AgentDecisionCommitter;
use trpg_shared_kernel::AuthorityMode;

fn main() {
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let _ = AgentDecisionCommitter::new(
        trpg_test_support::identity_verifier(),
        [contract],
    );
}
