use crate::rag_snapshot::{query_visible_chunks, RagChunk};
use trpg_shared_kernel::PrincipalScope;

pub fn query_working_memory(chunks: &[RagChunk], principal: &PrincipalScope) -> Vec<RagChunk> {
    query_visible_chunks(chunks, principal)
}
