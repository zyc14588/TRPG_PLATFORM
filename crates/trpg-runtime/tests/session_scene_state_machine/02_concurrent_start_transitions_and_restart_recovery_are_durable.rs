
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_start_transitions_and_restart_recovery_are_durable() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("02_session_scene_phases/01_setup_concurrency_and_restart.rs");
}
