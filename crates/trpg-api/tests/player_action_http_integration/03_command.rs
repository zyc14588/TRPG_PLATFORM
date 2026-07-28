
fn command(suffix: &str, expected_version: i64) -> Value {
    json!({
        "command_id": format!("command_{suffix}"),
        "idempotency_key": format!("idempotency_{suffix}"),
        "expected_version": expected_version,
        "correlation_id": format!("correlation_{suffix}"),
        "causation_id": format!("causation_{suffix}"),
        "trace_id": format!("trace_{suffix}")
    })
}

#[test]
fn player_action_http_path_authenticates_executes_rules_and_commits_atomically() {
    let primary_url = env::var("P07_DATABASE_URL").expect("P07_DATABASE_URL required");
    let witness_url =
        env::var("P07_WITNESS_DATABASE_URL").expect("P07_WITNESS_DATABASE_URL required");
    let nats_url = env::var("P07_NATS_URL").expect("P07_NATS_URL required");
    let primary_name = env::var("P07_RESET_DATABASE").unwrap();
    let witness_name = env::var("P07_WITNESS_RESET_DATABASE").unwrap();
    let setup_runtime = tokio::runtime::Runtime::new().unwrap();
    let primary = setup_runtime.block_on(reset_database(&primary_url, &primary_name, false));
    let witness = setup_runtime.block_on(reset_database(&witness_url, &witness_name, true));
    setup_runtime.block_on(witness.close());
    let store = setup_runtime
        .block_on(PostgresCanonicalStore::connect(
            &primary_url,
            &witness_url,
            "p07-http-integrity-key",
            INTEGRITY_KEY,
            "p07-http-payload-key",
            PAYLOAD_KEY,
        ))
        .unwrap();
    setup_runtime
        .block_on(store.prepare_for_service())
        .expect("P07 migrations apply to empty primary and witness databases");
    let canonical = store.clone();
    let repository = CoreDomainRepository::new(primary.clone(), store);
    setup_runtime.block_on(seed_tutorial(&repository, &primary));
    let (publisher, mut realtime_messages) = setup_runtime.block_on(async {
        let nats = async_nats::connect(&nats_url)
            .await
            .expect("connect to dedicated P07 JetStream");
        let jetstream = async_nats::jetstream::new(nats.clone());
        let _ = jetstream.delete_stream("TRPG_CANONICAL_EVENTS").await;
        let messages = nats
            .subscribe("trpg.events.appended.>")
            .await
            .expect("subscribe to P07 canonical realtime events");
        nats.flush()
            .await
            .expect("activate P07 realtime subscription");
        let publisher =
            JetStreamOutboxPublisher::connect(canonical, &nats_url, "p07-http-realtime", None)
                .await
                .expect("construct production P07 Outbox publisher");
        publisher
            .ensure_stream()
            .await
            .expect("create canonical P07 JetStream");
        (publisher, messages)
    });

    let audit_directory = tempfile::Builder::new()
        .prefix("trpg-p07-http-audit-")
        .tempdir()
        .unwrap();
    let audit_path = audit_directory.path().join("formal-audit.jsonl");
    let audit = FileAuditLog::open(&audit_path, "p07-http-audit-v1", &[0x69; 32]).unwrap();
    let (identity, tokens) = identity(now_unix_ms());
    let application = HttpPlayerActionApplication::new(
        identity,
        policy(),
        audit,
        RepositoryPlayerActionPort::new(repository),
    );
    let player_token = tokens.player;
    let keeper_token = tokens.keeper;
    let other_keeper_token = tokens.other_keeper;

    let invalid_action = "action_p07_http_client_dice";
    let event_count_before: i64 = setup_runtime.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM public.event_store")
            .fetch_one(&primary)
            .await
            .unwrap()
    });
    let (status, _) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions"),
            Some(&player_token),
            json!({
                "command": command("p07_http_client_dice", 0),
                "campaign_id": CAMPAIGN_ID,
                "action_id": invalid_action,
                "character_id": CHARACTER_ID,
                "scene_id": "scene_p07_http",
                "submitted_at_unix_ms": NOW_MS + 3_000,
                "intent": {
                    "kind": "INVESTIGATION",
                    "skill_name": "Library Use",
                    "clue_id": "clue_wrong_signature",
                    "clue_importance": "CORE",
                    "adjustment": "NONE",
                    "roll": 1
                }
            }),
        ),
    );
    assert_eq!(status, 400, "client-supplied dice must be rejected");
    let event_count_after: i64 = setup_runtime.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM public.event_store")
            .fetch_one(&primary)
            .await
            .unwrap()
    });
    assert_eq!(event_count_after, event_count_before);

    let submission = json!({
        "command": command("p07_http_submit", 0),
        "campaign_id": CAMPAIGN_ID,
        "action_id": ACTION_ID,
        "character_id": CHARACTER_ID,
        "scene_id": "scene_p07_http",
        "submitted_at_unix_ms": NOW_MS + 3_000,
        "intent": {
            "kind": "INVESTIGATION",
            "skill_name": "Library Use",
            "clue_id": "clue_wrong_signature",
            "clue_importance": "CORE",
            "adjustment": "NONE"
        }
    });
    let (status, submitted) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions"),
            Some(&player_token),
            submission,
        ),
    );
    assert_eq!(status, 202);
    assert_eq!(submitted["state"], "AWAITING_HUMAN_CONFIRMATION");
    assert!(submitted.get("rolled_value").is_none());
    let dice_before_confirmation: i64 = setup_runtime.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM public.dice_rolls")
            .fetch_one(&primary)
            .await
            .unwrap()
    });
    assert_eq!(dice_before_confirmation, 0);

    let (status, _) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions/{ACTION_ID}/confirm"),
            Some(&other_keeper_token),
            json!({
                "command": command("p07_http_non_owner_confirm", 1),
                "campaign_id": CAMPAIGN_ID,
                "action_id": ACTION_ID,
                "resolved_at_unix_ms": NOW_MS + 4_000
            }),
        ),
    );
    assert_eq!(status, 403);
    let dice_after_denial: i64 = setup_runtime.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM public.dice_rolls")
            .fetch_one(&primary)
            .await
            .unwrap()
    });
    assert_eq!(dice_after_denial, 0);

    let confirmation = json!({
        "command": command("p07_http_confirm", 1),
        "campaign_id": CAMPAIGN_ID,
        "action_id": ACTION_ID,
        "resolved_at_unix_ms": NOW_MS + 4_000
    });
    let (status, resolved) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions/{ACTION_ID}/confirm"),
            Some(&keeper_token),
            confirmation.clone(),
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(resolved["state"], "RESOLVED");
    assert!(resolved.get("rolled_value").is_none());
    let first_sequence = resolved["first_event_sequence"].as_i64().unwrap();
    let last_sequence = resolved["last_event_sequence"].as_i64().unwrap();
    assert_eq!(
        resolved["realtime_delta_id"],
        format!("delta_player_action_{last_sequence}")
    );

    let realtime_binding = setup_runtime.block_on(async {
        sqlx::query(
            "SELECT o.nats_subject, o.visibility_label, o.visibility_subject, \
                    o.correlation_id, e.trace_id, e.fact_provenance_kind, \
                    e.fact_provenance_reference \
               FROM public.event_outbox o \
               JOIN public.event_store e ON e.sequence = o.event_sequence \
              WHERE o.event_sequence = $1",
        )
        .bind(last_sequence)
        .fetch_one(&primary)
        .await
        .unwrap()
    });
    assert_eq!(
        realtime_binding.get::<String, _>("nats_subject"),
        "trpg.events.appended"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("visibility_label"),
        "party_visible"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("visibility_subject"),
        "not_applicable"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("correlation_id"),
        "correlation_p07_http_confirm"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("trace_id"),
        "trace_p07_http_confirm"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("fact_provenance_kind"),
        "human_keeper_statement"
    );
    assert_eq!(
        realtime_binding.get::<String, _>("fact_provenance_reference"),
        "command_p07_http_confirm"
    );

    let row = setup_runtime.block_on(async {
        sqlx::query(
            "SELECT d.target_value, d.rolled_value, d.random_source, \
                    c.importance, c.outcome, c.revealed_to_party \
             FROM public.dice_rolls d \
             JOIN public.clues c ON c.action_id = d.action_id \
             WHERE d.action_id = $1",
        )
        .bind(ACTION_ID)
        .fetch_one(&primary)
        .await
        .unwrap()
    });
    assert_eq!(row.get::<i16, _>("target_value"), 70);
    assert!((1..=100).contains(&row.get::<i16, _>("rolled_value")));
    assert_eq!(row.get::<String, _>("random_source"), "SERVER_OS_CSPRNG");
    assert_eq!(row.get::<String, _>("importance"), "CORE");
    assert!(matches!(
        row.get::<String, _>("outcome").as_str(),
        "REVEALED" | "REVEALED_WITH_COST"
    ));
    assert!(row.get::<bool, _>("revealed_to_party"));

    let (retry_status, retried) = exchange(
        application.clone(),
        json_request(
            "POST",
            &format!("/campaigns/{CAMPAIGN_ID}/player-actions/{ACTION_ID}/confirm"),
            Some(&keeper_token),
            confirmation,
        ),
    );
    assert_eq!(retry_status, 200);
    assert_eq!(retried["first_event_sequence"], first_sequence);
    assert_eq!(retried["last_event_sequence"], last_sequence);
    let counts = setup_runtime.block_on(async {
        sqlx::query(
            "SELECT \
               (SELECT count(*) FROM public.dice_rolls WHERE action_id = $1) AS dice, \
               (SELECT count(*) FROM public.decision_records WHERE action_id = $1) AS decisions, \
               (SELECT count(*) FROM public.clues WHERE action_id = $1) AS clues, \
               (SELECT count(*) FROM public.event_outbox \
                  WHERE commit_id = 'commit_command_p07_http_confirm') AS outbox",
        )
        .bind(ACTION_ID)
        .fetch_one(&primary)
        .await
        .unwrap()
    });
    assert_eq!(counts.get::<i64, _>("dice"), 1);
    assert_eq!(counts.get::<i64, _>("decisions"), 1);
    assert_eq!(counts.get::<i64, _>("clues"), 1);
    assert_eq!(counts.get::<i64, _>("outbox"), 4);

    let delivery = setup_runtime
        .block_on(publisher.publish_batch())
        .expect("publish P07 canonical Outbox through production JetStream adapter");
    assert_eq!(delivery.failed, 0);
    assert!(delivery.published >= 5);
    let mut delivered_action_delta = None;
    for _ in 0..delivery.published {
        let message = setup_runtime
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(5), realtime_messages.next()).await
            })
            .expect("P07 realtime delivery timed out")
            .expect("P07 realtime subscription ended");
        let envelope: EventEnvelopeWire<Value> =
            serde_json::from_slice(&message.payload).expect("canonical realtime envelope");
        if i64::try_from(envelope.sequence).ok() == Some(last_sequence) {
            delivered_action_delta = Some(envelope);
        }
    }
    let delivered_action_delta =
        delivered_action_delta.expect("HTTP realtime_delta_id must resolve to a NATS envelope");
    assert_eq!(delivered_action_delta.stream_id, ACTION_ID);
    assert_eq!(delivered_action_delta.event_type, "DecisionCommitted");
    assert_eq!(delivered_action_delta.visibility_label, "party_visible");
    assert_eq!(
        delivered_action_delta.correlation_id,
        "correlation_p07_http_confirm"
    );
    assert_eq!(
        delivered_action_delta.provenance_reference,
        "command_p07_http_confirm"
    );
    let published: bool = setup_runtime.block_on(async {
        sqlx::query_scalar(
            "SELECT published_at IS NOT NULL FROM public.event_outbox WHERE event_sequence = $1",
        )
        .bind(last_sequence)
        .fetch_one(&primary)
        .await
        .unwrap()
    });
    assert!(
        published,
        "Realtime ACK must close the transactional Outbox row"
    );
    setup_runtime.block_on(async move {
        drop(realtime_messages);
        drop(publisher);
    });

    drop(application);
    setup_runtime.block_on(primary.close());
    drop(audit_directory);
}
