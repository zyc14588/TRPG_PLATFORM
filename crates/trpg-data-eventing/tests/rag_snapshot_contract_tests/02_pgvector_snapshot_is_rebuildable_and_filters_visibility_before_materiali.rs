
#[tokio::test]
async fn pgvector_snapshot_is_rebuildable_and_filters_visibility_before_materialization() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("02_rag_snapshot_phases/01_materialize_and_query_visibility.rs");
}
