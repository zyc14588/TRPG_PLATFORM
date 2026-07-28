{

    let forced_rebuild = restarted
        .rebuild_from_genesis("campaign_projection_resume", "scene_projection_resume")
        .await
        .unwrap();
    assert_eq!(
        forced_rebuild.projection_hash,
        final_checkpoint.projection_hash
    );
    assert_projection_rows(&harness.primary, 5).await;

    // Database guards reject both cursor regression and hash equivocation at
    // an already committed cursor, including callers that bypass the worker.
    let backwards = sqlx::query(
        "UPDATE projection_checkpoint SET version = 4 WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3",
    )
    .bind("campaign_scene_projection")
    .bind("campaign_projection_resume")
    .bind("scene_projection_resume")
    .execute(&harness.primary)
    .await
    .expect_err("checkpoint regression must be rejected");
    assert!(backwards
        .to_string()
        .contains("projection checkpoint cannot move backwards"));
    let conflicting_hash = sqlx::query(
        "UPDATE projection_checkpoint SET projection_hash = $4 WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3",
    )
    .bind("campaign_scene_projection")
    .bind("campaign_projection_resume")
    .bind("scene_projection_resume")
    .bind("sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
    .execute(&harness.primary)
    .await
    .expect_err("same cursor with a different hash must be rejected");
    assert!(conflicting_hash
        .to_string()
        .contains("projection checkpoint hash conflicts at existing cursor"));

    let wrong_event_reference = sqlx::query(
        r#"
        INSERT INTO projection_checkpoint (
            projection_name, campaign_id, stream_id, version,
            last_event_sequence, projection_hash
        ) VALUES (
            'wrong_event_projection', 'campaign_projection_resume',
            'scene_projection_resume', 5, 9223372036854775806,
            'sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee'
        )
        "#,
    )
    .execute(&harness.primary)
    .await
    .expect_err("checkpoint cursor must reference the exact stream event");
    assert!(wrong_event_reference
        .to_string()
        .contains("projection checkpoint does not reference its canonical stream event"));

    // A caller-controlled search_path must not redirect the trigger to a
    // temporary lookalike table. The checkpoint must always bind to
    // public.event_store.
    let mut bypass = harness.primary.begin().await.unwrap();
    sqlx::raw_sql(
        r#"
        CREATE TEMP TABLE event_store (
            campaign_id TEXT,
            stream_id TEXT,
            stream_version BIGINT,
            sequence BIGINT
        );
        INSERT INTO event_store VALUES (
            'campaign_projection_resume', 'scene_projection_resume', 777, 888
        );
        SET LOCAL search_path = pg_temp, public;
        "#,
    )
    .execute(&mut *bypass)
    .await
    .unwrap();
    let forged_checkpoint = sqlx::query(
        r#"
        INSERT INTO public.projection_checkpoint (
            projection_name, campaign_id, stream_id, version,
            last_event_sequence, projection_hash
        ) VALUES (
            'search_path_probe', 'campaign_projection_resume',
            'scene_projection_resume', 777, 888,
            'sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd'
        )
        "#,
    )
    .execute(&mut *bypass)
    .await
    .expect_err("temporary event_store must not satisfy the canonical checkpoint guard");
    assert!(forged_checkpoint
        .to_string()
        .contains("projection checkpoint does not reference its canonical stream event"));
    bypass.rollback().await.unwrap();

    // Visibility, provenance, and payload are protected hash inputs. JSON map
    // key ordering is canonical, so equivalent payloads hash identically.
    let event = before_crash.events().first().unwrap().clone();
    let baseline_hash = hash_one(&event);
    let mut changed_visibility = event.clone();
    changed_visibility.visibility_label = "keeper_only".to_owned();
    assert_ne!(baseline_hash, hash_one(&changed_visibility));
    let mut changed_provenance = event.clone();
    changed_provenance.provenance_reference = "different_decision".to_owned();
    assert_ne!(baseline_hash, hash_one(&changed_provenance));
    let mut changed_provenance_kind = event.clone();
    changed_provenance_kind.provenance_kind = "tool_result".to_owned();
    assert_ne!(baseline_hash, hash_one(&changed_provenance_kind));
    let mut changed_provenance_actor = event.clone();
    changed_provenance_actor.provenance_recorded_by = "different_recorder".to_owned();
    assert_ne!(baseline_hash, hash_one(&changed_provenance_actor));
    let mut changed_request_hash = event.clone();
    changed_request_hash.request_hash =
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_owned();
    assert_ne!(baseline_hash, hash_one(&changed_request_hash));
    let mut changed_payload = event.clone();
    changed_payload.payload = json!({"changed": true});
    assert_ne!(baseline_hash, hash_one(&changed_payload));
    let mut ordered_a = event.clone();
    ordered_a.payload = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
    let mut ordered_b = event;
    ordered_b.payload = serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
    assert_eq!(hash_one(&ordered_a), hash_one(&ordered_b));

    let next_event = before_crash.events().get(1).unwrap();
    let uppercase_checkpoint = format!("sha256:{}", baseline_hash[7..].to_ascii_uppercase());
    let mut lower = CanonicalProjectionHasher::resume(&baseline_hash).unwrap();
    let mut upper = CanonicalProjectionHasher::resume(uppercase_checkpoint).unwrap();
    lower.apply(next_event).unwrap();
    upper.apply(next_event).unwrap();
    assert_eq!(lower.projection_hash(), upper.projection_hash());

    // The composite checkpoint identity independently tracks another stream.
    harness
        .store
        .commit(&draft(
            "campaign_projection_resume",
            "scene_projection_secondary",
            "projection_secondary",
            0,
            &["ProjectionResumeProbe"],
        ))
        .await
        .unwrap();
    let secondary = restarted
        .rebuild_to_tip("campaign_projection_resume", "scene_projection_secondary")
        .await
        .unwrap();
    assert_eq!(secondary.version, 1);
    let checkpoint_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM projection_checkpoint WHERE projection_name = 'campaign_scene_projection' AND campaign_id = 'campaign_projection_resume'",
    )
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    assert_eq!(checkpoint_count, 2);
}
