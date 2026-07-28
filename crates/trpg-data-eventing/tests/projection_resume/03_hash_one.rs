
fn hash_one(
    event: &trpg_data_eventing::event_store_sqlx_outbox_projection::CanonicalReplayEvent,
) -> String {
    let mut hasher = CanonicalProjectionHasher::default();
    hasher.apply(event).unwrap();
    hasher.projection_hash().to_owned()
}

async fn assert_projection_rows(pool: &sqlx::PgPool, expected: i64) {
    let rows: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
          FROM public.canonical_event_projection
         WHERE projection_name = 'campaign_scene_projection'
           AND campaign_id = 'campaign_projection_resume'
           AND stream_id = 'scene_projection_resume'
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(rows, expected);
}
