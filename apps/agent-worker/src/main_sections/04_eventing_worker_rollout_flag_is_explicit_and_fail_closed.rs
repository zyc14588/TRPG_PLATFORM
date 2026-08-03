
#[cfg(test)]
mod tests {
    use super::{
        background_cycle_error, parse_boolean_environment_value, require_eventing_workers_enabled,
        BackgroundWorkerHealth,
    };
    use std::time::{Duration, Instant};
    use trpg_data_eventing::event_bus_nats_impl::{JetStreamOutboxError, PublishBatchResult};
    use trpg_data_eventing::campaign_export_worker::CampaignExportOutcome;

    #[test]
    fn eventing_worker_rollout_flag_is_explicit_and_fail_closed() {
        assert_eq!(
            parse_boolean_environment_value("TRPG_P04_EVENTING_WORKERS_ENABLED", "true"),
            Ok(true)
        );
        assert_eq!(
            parse_boolean_environment_value("TRPG_P04_EVENTING_WORKERS_ENABLED", "0"),
            Ok(false)
        );
        assert_eq!(require_eventing_workers_enabled(true), Ok(()));
        assert_eq!(
            require_eventing_workers_enabled(false),
            Err("TRPG_P04_EVENTING_WORKERS_DISABLED_FAIL_CLOSED".to_owned())
        );
        assert_eq!(
            parse_boolean_environment_value("TRPG_P04_EVENTING_WORKERS_ENABLED", "enabled"),
            Err("TRPG_P04_EVENTING_WORKERS_ENABLED_MUST_BE_BOOLEAN".to_owned())
        );
    }

    #[test]
    fn delivery_projection_and_deletion_health_fail_independently() {
        let healthy_delivery = Ok(PublishBatchResult::default());
        let healthy_deletion = Ok(Vec::new());
        let healthy_export = Ok(CampaignExportOutcome::Idle);
        assert_eq!(
            background_cycle_error(
                &healthy_delivery,
                &Ok(1),
                &healthy_deletion,
                &healthy_export,
            ),
            None
        );

        let delivery_failed = Err(JetStreamOutboxError::NatsUnavailable);
        assert_eq!(
            background_cycle_error(
                &delivery_failed,
                &Ok(1),
                &healthy_deletion,
                &healthy_export,
            ),
            Some("EVENTING_DELIVERY_CYCLE_FAILED:NATS unavailable".to_owned())
        );

        let projection_failed: Result<(), JetStreamOutboxError> =
            Err(JetStreamOutboxError::Database("projection_rebuild"));
        assert_eq!(
            background_cycle_error(
                &healthy_delivery,
                &projection_failed,
                &healthy_deletion,
                &healthy_export,
            ),
            Some("PROJECTION_REBUILD_FAILED:outbox database failed: projection_rebuild".to_owned())
        );
        let deletion_failed =
            Err(trpg_security_governance::security_privacy::PrivacyError::Database);
        assert_eq!(
            background_cycle_error(
                &healthy_delivery,
                &Ok(1),
                &deletion_failed,
                &healthy_export,
            ),
            Some("PRIVACY_DELETION_CYCLE_FAILED:PRIVACY_DATABASE_ERROR".to_owned())
        );
        let combined = background_cycle_error(
            &delivery_failed,
            &projection_failed,
            &deletion_failed,
            &healthy_export,
        )
        .unwrap();
        assert!(combined.contains("EVENTING_DELIVERY_CYCLE_FAILED"));
        assert!(combined.contains("PROJECTION_REBUILD_FAILED"));
        assert!(combined.contains("PRIVACY_DELETION_CYCLE_FAILED"));
    }

    #[test]
    fn background_health_rejects_pending_stale_and_stopped_workers() {
        let started = Instant::now();
        let mut health = BackgroundWorkerHealth::default();
        assert_eq!(
            health.readiness_error(started, Duration::from_secs(30)),
            Some("AGENT_WORKER_DEPENDENCY_CHECK_PENDING".to_owned())
        );

        health.record_cycle(started, None);
        assert_eq!(
            health.readiness_error(started + Duration::from_secs(30), Duration::from_secs(30)),
            None
        );
        assert_eq!(
            health.readiness_error(started + Duration::from_secs(31), Duration::from_secs(30),),
            Some("AGENT_WORKER_BACKGROUND_HEARTBEAT_STALE".to_owned())
        );

        health.record_stopped("AGENT_WORKER_BACKGROUND_PANICKED");
        assert_eq!(
            health.readiness_error(started, Duration::from_secs(30)),
            Some("AGENT_WORKER_BACKGROUND_PANICKED".to_owned())
        );
    }

    #[test]
    fn background_health_preserves_current_cycle_failure() {
        let completed_at = Instant::now();
        let mut health = BackgroundWorkerHealth::default();
        health.record_cycle(
            completed_at,
            Some("EVENTING_DELIVERY_CYCLE_FAILED:NATS unavailable".to_owned()),
        );

        assert_eq!(
            health.readiness_error(completed_at, Duration::from_secs(30)),
            Some("EVENTING_DELIVERY_CYCLE_FAILED:NATS unavailable".to_owned())
        );
    }
}
