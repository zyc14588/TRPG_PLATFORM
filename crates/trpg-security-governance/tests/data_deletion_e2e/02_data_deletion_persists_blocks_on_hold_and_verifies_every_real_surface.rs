
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn data_deletion_persists_blocks_on_hold_and_verifies_every_real_surface() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("02_data_deletion_phases/01_private_surface_setup_and_holds.rs");
}
