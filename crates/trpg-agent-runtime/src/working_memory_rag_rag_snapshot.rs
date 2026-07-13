use crate::rag_snapshot::RagChunk;

pub fn validate_working_memory_snapshot(chunks: &[RagChunk]) -> bool {
    chunks.iter().all(RagChunk::has_required_metadata)
}
