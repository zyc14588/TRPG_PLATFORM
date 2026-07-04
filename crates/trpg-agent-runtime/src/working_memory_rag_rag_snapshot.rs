use crate::rag_snapshot::RagChunk;

pub const PROMPT_ID: &str = "CODEX-0452-04-AI-AGENT-SYSTEM-4f2dab7f75";

pub fn validate_working_memory_snapshot(chunks: &[RagChunk]) -> bool {
    chunks.iter().all(RagChunk::has_required_metadata)
}
