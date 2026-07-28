
#[tokio::test(flavor = "multi_thread")]
async fn outbox_waits_for_jetstream_ack_and_redis_remains_a_versioned_read_model() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("02_jetstream_redis_phases/01_migration_publish_and_durability.rs");
}
