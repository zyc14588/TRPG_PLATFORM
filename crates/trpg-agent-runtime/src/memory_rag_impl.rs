use crate::agent_runtime::{AgentError, AgentResult, AssembledAgentContext, ContextFact};
use crate::rag_snapshot::RagChunk;
use trpg_identity::{IdentityError, ReplayAuthorization};
use trpg_shared_kernel::{EventStore, TrpgError, Visibility, VisibilityLabel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryRagView {
    pub context: AssembledAgentContext,
    pub chunks: Vec<RagChunk>,
    pub visible_event_count: usize,
}

pub fn assemble_memory_rag_view<P: Clone>(
    facts: &[ContextFact],
    chunks: &[RagChunk],
    store: &EventStore<P>,
    processor_authorization: &ReplayAuthorization,
    target_authorization: &ReplayAuthorization,
    now_unix_ms: u64,
) -> AgentResult<MemoryRagView> {
    if processor_authorization.campaign_id() != target_authorization.campaign_id() {
        return Err(AgentError::Core(TrpgError::AuthorizationDenied));
    }
    // Both the process reading the material and the final target must be
    // authorized. A privileged target can no longer launder Keeper-only
    // chunks through a less-privileged processor.
    let mut visible_facts = Vec::new();
    for fact in facts {
        if both_can_view(
            processor_authorization,
            target_authorization,
            processor_authorization.campaign_id(),
            &fact.visibility,
            now_unix_ms,
        )? {
            visible_facts.push(fact.clone());
        }
    }
    let derived_visibility = visible_facts
        .iter()
        .map(|fact| fact.visibility.clone())
        .reduce(|current, candidate| current.intersection(&candidate))
        .unwrap_or_else(|| Visibility::new(VisibilityLabel::Public));
    let context = AssembledAgentContext {
        strictest_visibility: derived_visibility.label().clone(),
        facts: visible_facts,
        derived_visibility,
    };

    let mut visible_chunks = Vec::new();
    for chunk in chunks {
        if both_can_view(
            processor_authorization,
            target_authorization,
            processor_authorization.campaign_id(),
            &chunk.visibility,
            now_unix_ms,
        )? {
            visible_chunks.push(chunk.clone());
        }
    }

    let mut visible_event_count = 0;
    for event in store.events() {
        if both_can_view(
            processor_authorization,
            target_authorization,
            &event.campaign_id,
            &event.visibility,
            now_unix_ms,
        )? {
            visible_event_count += 1;
        }
    }

    Ok(MemoryRagView {
        context,
        chunks: visible_chunks,
        visible_event_count,
    })
}

pub fn memory_rag_chunks_are_rebuildable(chunks: &[RagChunk]) -> bool {
    chunks.iter().all(RagChunk::has_required_metadata)
}

fn both_can_view(
    processor_authorization: &ReplayAuthorization,
    target_authorization: &ReplayAuthorization,
    campaign_id: &trpg_shared_kernel::EntityId,
    visibility: &Visibility,
    now_unix_ms: u64,
) -> AgentResult<bool> {
    if !can_view(
        processor_authorization,
        campaign_id,
        visibility,
        now_unix_ms,
    )? {
        return Ok(false);
    }
    can_view(target_authorization, campaign_id, visibility, now_unix_ms)
}

fn can_view(
    authorization: &ReplayAuthorization,
    campaign_id: &trpg_shared_kernel::EntityId,
    visibility: &Visibility,
    now_unix_ms: u64,
) -> AgentResult<bool> {
    authorization
        .can_view(campaign_id, visibility, now_unix_ms)
        .map_err(|error| match error {
            IdentityError::MembershipRequired
            | IdentityError::MembershipDenied
            | IdentityError::CampaignScopeMismatch => {
                AgentError::Core(TrpgError::AuthorizationDenied)
            }
            _ => AgentError::Core(TrpgError::AuthenticationRequired),
        })
}
