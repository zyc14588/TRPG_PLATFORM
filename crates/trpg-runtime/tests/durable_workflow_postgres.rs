use std::env;

use trpg_runtime::durable_workflow::{
    DurableWorkflowStore, WorkflowState, WorkflowStoreError, WorkflowTransitionDraft,
};

#[tokio::test(flavor = "multi_thread")]
async fn workflow_state_and_leases_survive_process_reconstruction() {
    let Ok(database_url) = env::var("P02_WORKFLOW_DATABASE_URL") else {
        eprintln!("skipped: set P02_WORKFLOW_DATABASE_URL for the real durable-workflow gate");
        return;
    };
    let suffix = std::process::id();
    let workflow_id = format!("durable_workflow_{suffix}");
    let campaign_id = format!("durable_campaign_{suffix}");

    let store = DurableWorkflowStore::connect(&database_url).await.unwrap();
    store.apply_migration().await.unwrap();
    store.check_readiness().await.unwrap();
    let created = store
        .create(
            &workflow_id,
            &campaign_id,
            "keeper_turn",
            r#"{"step":"start"}"#,
            Some(1_000),
        )
        .await
        .unwrap();
    assert_eq!(created.state, WorkflowState::Pending);
    assert_eq!(created.version, 0);

    let leased = store
        .acquire_due("worker_a", 1_000, 5_000, 10)
        .await
        .unwrap();
    assert!(leased
        .iter()
        .any(|workflow| workflow.workflow_id == workflow_id));

    let transition = WorkflowTransitionDraft {
        workflow_id: workflow_id.clone(),
        expected_version: 0,
        from_state: WorkflowState::Pending,
        to_state: WorkflowState::Running,
        idempotency_key: format!("transition_start_{suffix}"),
        correlation_id: format!("correlation_start_{suffix}"),
        causation_id: format!("causation_start_{suffix}"),
        output_json: None,
        wake_at_unix_ms: None,
    };
    let started = store.transition(&transition).await.unwrap();
    assert_eq!(started.workflow_version, 1);
    assert_eq!(started.to_state, WorkflowState::Running);
    drop(store);

    let restarted = DurableWorkflowStore::connect(&database_url).await.unwrap();
    let restored = restarted.load(&workflow_id).await.unwrap().unwrap();
    assert_eq!(restored.state, WorkflowState::Running);
    assert_eq!(restored.version, 1);
    assert_eq!(restored.lease_owner.as_deref(), Some("worker_a"));

    // Replaying the same transition is idempotent and does not advance again.
    assert_eq!(restarted.transition(&transition).await.unwrap(), started);
    let conflict = WorkflowTransitionDraft {
        idempotency_key: format!("transition_conflict_{suffix}"),
        ..transition.clone()
    };
    assert!(matches!(
        restarted.transition(&conflict).await,
        Err(WorkflowStoreError::VersionConflict {
            expected: 0,
            actual: 1
        })
    ));

    // An expired lease can be reclaimed after a worker crash.
    let reclaimed = restarted
        .acquire_due("worker_b", 6_001, 5_000, 10)
        .await
        .unwrap();
    assert!(reclaimed.iter().any(|workflow| {
        workflow.workflow_id == workflow_id && workflow.lease_owner.as_deref() == Some("worker_b")
    }));

    let completed = restarted
        .transition(&WorkflowTransitionDraft {
            workflow_id: workflow_id.clone(),
            expected_version: 1,
            from_state: WorkflowState::Running,
            to_state: WorkflowState::Completed,
            idempotency_key: format!("transition_complete_{suffix}"),
            correlation_id: format!("correlation_complete_{suffix}"),
            causation_id: format!("causation_complete_{suffix}"),
            output_json: Some(r#"{"result":"done"}"#.to_owned()),
            wake_at_unix_ms: None,
        })
        .await
        .unwrap();
    assert_eq!(completed.workflow_version, 2);
    assert_eq!(
        restarted.load(&workflow_id).await.unwrap().unwrap().state,
        WorkflowState::Completed
    );
}
