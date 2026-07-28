
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn core_domain_schema_and_repository_are_event_backed_and_constrained() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("02_core_domain_schema_phases/01_database_and_schema_guards.rs");
}
