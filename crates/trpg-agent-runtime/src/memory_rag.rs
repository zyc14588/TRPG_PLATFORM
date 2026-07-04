use crate::rag_snapshot::{query_visible_chunks, RagChunk};
use trpg_shared_kernel::PrincipalScope;

pub const PROMPT_ID: &str = "CODEX-0456-04-AI-AGENT-SYSTEM-d68068a022";

pub fn query_memory_rag(chunks: &[RagChunk], principal: &PrincipalScope) -> Vec<RagChunk> {
    query_visible_chunks(chunks, principal)
}
