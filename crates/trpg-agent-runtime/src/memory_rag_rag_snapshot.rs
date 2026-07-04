use crate::rag_snapshot::RagChunk;

pub const PROMPT_ID: &str = "CODEX-0045-04-AI-AGENT-SYSTEM-e852321d0b";

pub fn validate_memory_rag_snapshot(chunks: &[RagChunk]) -> bool {
    chunks.iter().all(RagChunk::has_required_metadata)
}
