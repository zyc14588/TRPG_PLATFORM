
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tutorial_runs_through_real_repository_event_store_outbox_and_witness() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("03_tutorial_complete_phases/01_setup_and_investigation.rs");
}
