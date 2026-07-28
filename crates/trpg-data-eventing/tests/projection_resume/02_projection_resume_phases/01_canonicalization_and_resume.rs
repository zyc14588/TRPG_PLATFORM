{
    let harness = P04PostgresHarness::reset().await;
    let unicode_document = json!({
        "z": [true, false, null, 17],
        "e\u{301}": {"nested_z": "decomposed", "nested_ä": "umlaut"},
        "ä": {"A": "ascii", "中": "han"},
        "é": "composed",
        "中": {"Ω": "omega", "🙂": "emoji"}
    });
    let rust_canonical = serde_json::to_string(&unicode_document).unwrap();
    let postgres_canonical: String =
        sqlx::query_scalar("SELECT public.canonical_projection_json($1::jsonb)")
            .bind(sqlx::types::Json(unicode_document))
            .fetch_one(&harness.primary)
            .await
            .unwrap();
    assert_eq!(
        postgres_canonical, rust_canonical,
        "PostgreSQL C-collated object ordering must match Rust canonical JSON for Unicode keys"
    );
    for version in 0..5 {
        harness
            .store
            .commit(&draft(
                "campaign_projection_resume",
                "scene_projection_resume",
                &format!("projection_resume_{version}"),
                version,
                &["ProjectionResumeProbe"],
            ))
            .await
            .unwrap();
    }

    let worker =
        PostgresProjectionWorker::new(harness.primary.clone(), "campaign_scene_projection", 2)
            .unwrap();

    // Simulate a crash after deterministic page application but before the
    // checkpoint transaction. A reconstructed worker computes the same page
    // and hash, proving it can safely repeat the work.
    let before_crash = worker
        .prepare_page("campaign_projection_resume", "scene_projection_resume")
        .await
        .unwrap();
    assert_eq!(before_crash.start().version, 0);
    assert_eq!(before_crash.target().version, 2);
    drop(worker);
    let restarted =
        PostgresProjectionWorker::new(harness.primary.clone(), "campaign_scene_projection", 2)
            .unwrap();
    let after_crash = restarted
        .prepare_page("campaign_projection_resume", "scene_projection_resume")
        .await
        .unwrap();
    assert_eq!(after_crash.events(), before_crash.events());
    assert_eq!(after_crash.target().version, before_crash.target().version);
    assert_eq!(
        after_crash.target().last_event_sequence,
        before_crash.target().last_event_sequence
    );
    assert_eq!(
        after_crash.target().projection_hash,
        before_crash.target().projection_hash
    );

    // Projection rows and their checkpoint are one atomic unit. A failure at
    // the checkpoint boundary must roll back rows already applied in the same
    // transaction.
    sqlx::raw_sql(
        r#"
        CREATE FUNCTION p04_reject_checkpoint_probe()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            RAISE EXCEPTION 'p04 checkpoint failure probe';
        END;
        $$;
        CREATE TRIGGER p04_reject_checkpoint_probe
        BEFORE INSERT OR UPDATE ON public.projection_checkpoint
        FOR EACH ROW EXECUTE FUNCTION p04_reject_checkpoint_probe();
        "#,
    )
    .execute(&harness.primary)
    .await
    .unwrap();
    restarted
        .advance_checkpoint(&after_crash)
        .await
        .expect_err("checkpoint failure must abort projection application");
    let rolled_back_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.canonical_event_projection WHERE projection_name = 'campaign_scene_projection'",
    )
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    assert_eq!(rolled_back_rows, 0);
    sqlx::raw_sql(
        r#"
        DROP TRIGGER p04_reject_checkpoint_probe ON public.projection_checkpoint;
        DROP FUNCTION p04_reject_checkpoint_probe();
        "#,
    )
    .execute(&harness.primary)
    .await
    .unwrap();
    assert!(matches!(
        restarted.advance_checkpoint(&after_crash).await.unwrap(),
        CheckpointAdvance::Advanced(ref checkpoint) if checkpoint.version == 2
    ));
    let first_page_projection_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canonical_event_projection WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3",
    )
    .bind("campaign_scene_projection")
    .bind("campaign_projection_resume")
    .bind("scene_projection_resume")
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    assert_eq!(first_page_projection_rows, 2);
    let protected_projection = sqlx::query(
        r#"
        SELECT event_document->>'visibility_label' AS visibility_label,
               event_document->>'provenance_reference' AS provenance_reference,
               event_document->>'correlation_id' AS correlation_id,
               event_document->>'causation_id' AS causation_id
          FROM public.canonical_event_projection
         WHERE projection_name = 'campaign_scene_projection'
         ORDER BY stream_version
         LIMIT 1
        "#,
    )
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    assert_eq!(
        protected_projection.get::<String, _>("visibility_label"),
        "party_visible"
    );
    assert!(protected_projection
        .get::<String, _>("provenance_reference")
        .starts_with("decision_projection_resume_"));
    assert!(protected_projection
        .get::<String, _>("correlation_id")
        .starts_with("correlation_projection_resume_"));
    assert!(protected_projection
        .get::<String, _>("causation_id")
        .starts_with("causation_projection_resume_"));

    // A correct Event Store-derived document paired with an attacker-chosen
    // hash used to pass every database guard. The database must independently
    // derive the v3 chain link and reject the forged digest.
    let forged_projection_hash = sqlx::query(
        r#"
        INSERT INTO public.canonical_event_projection (
            projection_name, campaign_id, stream_id, stream_version,
            event_sequence, projection_hash, event_document
        )
        SELECT 'forged_hash_projection', campaign_id, stream_id,
               stream_version, event_sequence,
               'sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
               event_document
          FROM public.canonical_event_projection
         WHERE projection_name = 'campaign_scene_projection'
           AND campaign_id = 'campaign_projection_resume'
           AND stream_id = 'scene_projection_resume'
           AND stream_version = 1
        "#,
    )
    .execute(&harness.primary)
    .await
    .expect_err("projection hash must be derived from the canonical Event Store chain");
    assert!(forged_projection_hash
        .to_string()
        .contains("canonical projection hash does not match Event Store chain"));

    let forged_initial_checkpoint = sqlx::query(
        r#"
        INSERT INTO public.projection_checkpoint (
            projection_name, campaign_id, stream_id, version,
            last_event_sequence, projection_hash
        )
        SELECT 'forged_checkpoint_projection', campaign_id, stream_id,
               stream_version, event_sequence,
               'sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'
          FROM public.canonical_event_projection
         WHERE projection_name = 'campaign_scene_projection'
           AND campaign_id = 'campaign_projection_resume'
           AND stream_id = 'scene_projection_resume'
           AND stream_version = 1
        "#,
    )
    .execute(&harness.primary)
    .await
    .expect_err("checkpoint must bind to a database-verified projection row");
    assert!(forged_initial_checkpoint
        .to_string()
        .contains("projection checkpoint does not match materialized Event Store chain"));

    // Two workers racing the same prepared page serialize on the composite
    // checkpoint identity. One advances and one observes the exact result.
    let concurrent_page = restarted
        .prepare_page("campaign_projection_resume", "scene_projection_resume")
        .await
        .unwrap();
    assert_eq!(concurrent_page.target().version, 4);
    let peer =
        PostgresProjectionWorker::new(harness.primary.clone(), "campaign_scene_projection", 2)
            .unwrap();
    let (first_advance, second_advance) = tokio::join!(
        restarted.advance_checkpoint(&concurrent_page),
        peer.advance_checkpoint(&concurrent_page)
    );
    let advances = [first_advance.unwrap(), second_advance.unwrap()];
    assert_eq!(
        advances
            .iter()
            .filter(|result| matches!(result, CheckpointAdvance::Advanced(_)))
            .count(),
        1
    );
    assert_eq!(
        advances
            .iter()
            .filter(|result| matches!(result, CheckpointAdvance::AlreadyApplied(_)))
            .count(),
        1
    );
    let final_checkpoint = restarted
        .rebuild_to_tip("campaign_projection_resume", "scene_projection_resume")
        .await
        .unwrap();
    assert_eq!(final_checkpoint.version, 5);
    assert_eq!(
        restarted
            .checkpoint("campaign_projection_resume", "scene_projection_resume")
            .await
            .unwrap(),
        final_checkpoint
    );

    // A projection row cannot be forged or changed in place. Missing rows,
    // however, are a supported recovery condition because this table is a
    // rebuildable read model rather than canonical history.
    let mutation = sqlx::query(
        r#"
        UPDATE public.canonical_event_projection
           SET event_document = jsonb_set(
               event_document, '{payload}', '{"forged":true}'::jsonb
           )
         WHERE projection_name = $1
           AND campaign_id = $2
           AND stream_id = $3
           AND stream_version = 3
        "#,
    )
    .bind("campaign_scene_projection")
    .bind("campaign_projection_resume")
    .bind("scene_projection_resume")
    .execute(&harness.primary)
    .await
    .expect_err("materialized canonical events must be immutable");
    assert!(mutation
        .to_string()
        .contains("canonical event projection rows are immutable"));

    // Retaining a tip checkpoint after deleting its read-model rows used to
    // return NoEvents and falsely report success. It must now reset only the
    // downstream state and reconstruct all five rows from Event Store.
    sqlx::query(
        r#"
        DELETE FROM public.canonical_event_projection
         WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3
        "#,
    )
    .bind("campaign_scene_projection")
    .bind("campaign_projection_resume")
    .bind("scene_projection_resume")
    .execute(&harness.primary)
    .await
    .unwrap();
    let repaired_missing_rows = restarted
        .rebuild_to_tip("campaign_projection_resume", "scene_projection_resume")
        .await
        .unwrap();
    assert_eq!(
        repaired_missing_rows.projection_hash,
        final_checkpoint.projection_hash
    );
    assert_projection_rows(&harness.primary, 5).await;

    // The inverse damage (rows retained, checkpoint deleted) must also start
    // from a clean genesis instead of conflicting on projection primary keys.
    sqlx::query(
        r#"
        DELETE FROM public.projection_checkpoint
         WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3
        "#,
    )
    .bind("campaign_scene_projection")
    .bind("campaign_projection_resume")
    .bind("scene_projection_resume")
    .execute(&harness.primary)
    .await
    .unwrap();
    let repaired_missing_checkpoint = restarted
        .rebuild_to_tip("campaign_projection_resume", "scene_projection_resume")
        .await
        .unwrap();
    assert_eq!(
        repaired_missing_checkpoint.projection_hash,
        final_checkpoint.projection_hash
    );
    assert_projection_rows(&harness.primary, 5).await;

    // A partial hole is detected by the checkpoint/materialization invariant
    // and repaired through the same full, deterministic replay.
    sqlx::query(
        r#"
        DELETE FROM public.canonical_event_projection
         WHERE projection_name = $1 AND campaign_id = $2 AND stream_id = $3
           AND stream_version = 3
        "#,
    )
    .bind("campaign_scene_projection")
    .bind("campaign_projection_resume")
    .bind("scene_projection_resume")
    .execute(&harness.primary)
    .await
    .unwrap();
    let repaired_partial_projection = restarted
        .rebuild_to_tip("campaign_projection_resume", "scene_projection_resume")
        .await
        .unwrap();
    assert_eq!(
        repaired_partial_projection.projection_hash,
        final_checkpoint.projection_hash
    );
    assert_projection_rows(&harness.primary, 5).await;
    include!("02_forced_rebuild_and_checkpoint_guards.rs");
}
