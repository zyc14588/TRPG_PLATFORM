use crate::agent_runtime::{assemble_context, AssembledAgentContext, ContextFact};
use crate::rag_snapshot::{query_visible_chunks, RagChunk};
use trpg_shared_kernel::{EventStore, PrincipalScope};

pub const PROMPT_ID: &str = "CODEX-0483-04-AI-AGENT-SYSTEM-a577767984";

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
    principal: &PrincipalScope,
) -> MemoryRagView {
    MemoryRagView {
        context: assemble_context(facts, principal),
        chunks: query_visible_chunks(chunks, principal),
        visible_event_count: store.replay_visible(principal).len(),
    }
}

pub fn memory_rag_chunks_are_rebuildable(chunks: &[RagChunk]) -> bool {
    chunks.iter().all(RagChunk::has_required_metadata)
}
