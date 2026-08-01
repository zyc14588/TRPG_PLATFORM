
impl AgentWorkerProcess {
    fn start(self) -> Result<(RoleRuntimeProbe, BackgroundWorker), String> {
        let background_health = Arc::new(Mutex::new(BackgroundWorkerHealth::default()));
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();
        let background_outbox = self.outbox.clone();
        let background_workflow = self.workflow.clone();
        let background_deletion = self.deletion;
        let background_campaign_exports = self.campaign_exports;
        let background_agent_jobs = self.agent_jobs;
        let background_runtime = self.runtime;
        let background_health_writer = Arc::clone(&background_health);
        let worker = thread::Builder::new()
            .name("agent-outbox-publisher".to_owned())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop {
                    let (delivery, projection, deletion, export, agent) =
                        background_runtime.block_on(async {
                        let agent = match &background_agent_jobs {
                            Some(agent_jobs) => {
                                agent_jobs.run_once(current_unix_ms()).await
                            }
                            None => Ok(AgentJobOutcome::Idle),
                        };
                        let delivery = async {
                            background_workflow.check_readiness().await.map_err(|_| {
                                trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxError::Database(
                                    "workflow_readiness",
                                )
                            })?;
                            background_outbox.stream_message_count().await?;
                            background_outbox.publish_batch().await
                        }
                        .await;
                        // Projection recovery is an independent Event Store read-model
                        // responsibility. A NATS outage must not prevent it from
                        // converging, and its failure must retain a distinct health
                        // classification from event delivery.
                        let projection = background_outbox.rebuild_projections_to_tip().await;
                        // Deletion is a durable, evidence-gated workflow. Its failures
                        // are independent of delivery/projection failures and therefore
                        // receive a distinct fail-closed health classification.
                        let deletion = background_deletion.execute_next(25).await;
                        let export = background_campaign_exports.run_once(current_unix_ms()).await;
                        (delivery, projection, deletion, export, agent)
                    });
                    if let Ok(outcome) = &export {
                        match outcome {
                            CampaignExportOutcome::RetryScheduled { export_id, error_code }
                            | CampaignExportOutcome::TerminalFailure { export_id, error_code } => {
                                eprintln!(
                                    "service=agent-worker campaign_export_id={export_id} error={error_code}"
                                );
                            }
                            CampaignExportOutcome::Idle
                            | CampaignExportOutcome::Completed { .. }
                            | CampaignExportOutcome::Expired { .. } => {}
                        }
                    }
                    if let Ok(outcome) = &agent {
                        match outcome {
                            AgentJobOutcome::RetryScheduled {
                                job_id,
                                error_code,
                            }
                            | AgentJobOutcome::TerminalFailure {
                                job_id,
                                error_code,
                            } => eprintln!(
                                "service=agent-worker agent_job_id={job_id} error={error_code}"
                            ),
                            AgentJobOutcome::Idle
                            | AgentJobOutcome::Completed { .. }
                            | AgentJobOutcome::AwaitingHumanApproval { .. } => {}
                        }
                    }
                    if let Ok(result) = &delivery {
                        if result.requires_operator_attention() {
                            let alert = result.alert_code().unwrap_or("OUTBOX_DELIVERY_ALERT");
                            eprintln!(
                                "service=agent-worker alert={alert} dead_lettered={} dead_letter_total={} failed={} claimed={}",
                                result.dead_lettered,
                                result.dead_letter_total,
                                result.failed,
                                result.claimed
                            );
                        }
                    }
                    if let Ok(mut health) = background_health_writer.lock() {
                        let mut error =
                            background_cycle_error(&delivery, &projection, &deletion, &export);
                        if let Err(agent_error) = &agent {
                            let agent_error =
                                format!("AGENT_JOB_CYCLE_FAILED:{}", agent_error.code());
                            error = Some(match error {
                                Some(existing) => format!("{existing};{agent_error}"),
                                None => agent_error,
                            });
                        }
                        health.record_cycle(Instant::now(), error);
                    }
                    match shutdown_receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }));
                if let Ok(mut health) = background_health_writer.lock() {
                    health.record_stopped(if outcome.is_ok() {
                        "AGENT_WORKER_BACKGROUND_STOPPED"
                    } else {
                        "AGENT_WORKER_BACKGROUND_PANICKED"
                    });
                }
            })
            .map_err(|_| "OUTBOX_WORKER_START_FAILED".to_owned())?;

        let plugins = self.plugins;
        let model_route = self.model_route;
        let probe_health = Arc::clone(&background_health);
        let probe = RoleRuntimeProbe::spawn("agent_worker_runtime", move || {
            let boundary = trpg_agent_runtime::provider_boundary_snapshot();
            if boundary.gateway != "Agent Gateway"
                || boundary.runtime != "Agent Orchestrator/Runtime"
                || boundary.provider_adapter != "Model Provider Adapter"
                || boundary.forbidden_direct_call_error != "DIRECT_LLM_CALL_FORBIDDEN"
            {
                return Err("provider boundary initialization is incomplete".to_owned());
            }
            let (provider_status, agent_job_status) =
                if let Some(model_route) = &model_route {
                    if model_route.fallback_policy
                        != "none_no_automatic_fallback"
                        || model_route.privacy_boundary
                            != "explicit_route_authorization_event"
                    {
                        return Err(
                            "model provider route authorization is incomplete"
                                .to_owned(),
                        );
                    }
                    (model_route.provider_type.route_name(), "ready")
                } else {
                    ("not_configured", "disabled_no_provider")
                };
            if let Some(error) = probe_health
                .lock()
                .map_err(|_| "outbox health lock poisoned".to_owned())?
                .readiness_error(Instant::now(), BACKGROUND_HEARTBEAT_STALE_AFTER)
            {
                return Err(error);
            }
            plugins.check_readiness()?;
            Ok(format!(
                "gateway/runtime/provider adapter ready; provider_status={}; durable_agent_jobs_status={}; workflow and sandboxed plugins ready; eventing_workers_status=enabled; plugins={}",
                provider_status,
                agent_job_status,
                plugins.plugin_count(),
            ))
        })
        .map_err(|error| error.to_string())?;
        Ok((
            probe,
            BackgroundWorker {
                shutdown_sender,
                worker: Some(worker),
            },
        ))
    }
}
