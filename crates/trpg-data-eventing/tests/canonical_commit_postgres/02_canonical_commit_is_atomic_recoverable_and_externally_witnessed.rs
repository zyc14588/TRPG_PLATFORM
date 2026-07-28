
#[tokio::test(flavor = "multi_thread")]
async fn canonical_commit_is_atomic_recoverable_and_externally_witnessed() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("02_canonical_commit_phases/01_setup_commit_and_sync_port.rs");
}
