use crate::rag_snapshot::{query_visible_chunks, RagChunk};
use trpg_shared_kernel::PrincipalScope;

pub const PROMPT_ID: &str = "CODEX-0448-04-AI-AGENT-SYSTEM-41ecd49e88";

pub fn query_working_memory(chunks: &[RagChunk], principal: &PrincipalScope) -> Vec<RagChunk> {
    query_visible_chunks(chunks, principal)
}
