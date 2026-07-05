use crate::{append_coc7_event, Coc7EventPayload};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, PrincipalScope,
    Visibility, VisibilityLabel,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcSecret {
    pub npc_id: String,
    pub public_name: String,
    pub keeper_secret: String,
    pub visibility: Visibility,
}

impl NpcSecret {
    pub fn keeper_only(
        npc_id: impl Into<String>,
        public_name: impl Into<String>,
        keeper_secret: impl Into<String>,
    ) -> Self {
        Self {
            npc_id: npc_id.into(),
            public_name: public_name.into(),
            keeper_secret: keeper_secret.into(),
            visibility: Visibility::new(VisibilityLabel::KeeperOnly),
        }
    }
}

pub fn npc_secret_for_principal<'a>(
    secret: &'a NpcSecret,
    principal: &PrincipalScope,
) -> Option<&'a str> {
    secret
        .visibility
        .can_view(principal)
        .then_some(secret.keeper_secret.as_str())
}

pub fn record_npc_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    secret: &NpcSecret,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.npc_decision_recorded",
        "npc",
        format!(
            "npc={} visibility={}",
            secret.npc_id,
            secret.visibility.label().as_str()
        ),
    )
}
