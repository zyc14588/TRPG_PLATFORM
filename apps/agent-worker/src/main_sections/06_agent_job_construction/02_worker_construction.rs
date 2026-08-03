fn map_agent_workflow_error(
    error: trpg_runtime::durable_workflow::WorkflowStoreError,
) -> AgentJobError {
    match error {
        trpg_runtime::durable_workflow::WorkflowStoreError::Connection
        | trpg_runtime::durable_workflow::WorkflowStoreError::Database(_) => {
            AgentJobError::retryable("AGENT_JOB_DATABASE_UNAVAILABLE")
        }
        trpg_runtime::durable_workflow::WorkflowStoreError::NotFound => {
            AgentJobError::terminal("AGENT_JOB_NOT_FOUND")
        }
        trpg_runtime::durable_workflow::WorkflowStoreError::VersionConflict { .. }
        | trpg_runtime::durable_workflow::WorkflowStoreError::StateConflict => {
            AgentJobError::retryable("AGENT_JOB_CONCURRENT_UPDATE")
        }
        trpg_runtime::durable_workflow::WorkflowStoreError::IdempotencyConflict => {
            AgentJobError::terminal("AGENT_JOB_IDEMPOTENCY_CONFLICT")
        }
        trpg_runtime::durable_workflow::WorkflowStoreError::Configuration(_)
        | trpg_runtime::durable_workflow::WorkflowStoreError::Migration
        | trpg_runtime::durable_workflow::WorkflowStoreError::Validation(_)
        | trpg_runtime::durable_workflow::WorkflowStoreError::IntegrityViolation(_) => {
            AgentJobError::terminal("AGENT_JOB_STORAGE_INTEGRITY_ERROR")
        }
    }
}

fn map_agent_canonical_context_error(error: CanonicalStoreError) -> AgentJobError {
    match error {
        CanonicalStoreError::Connection { .. }
        | CanonicalStoreError::WitnessWrite { .. }
        | CanonicalStoreError::WitnessFinalizationPending { .. } => {
            AgentJobError::retryable("AGENT_CANONICAL_INPUT_UNAVAILABLE")
        }
        CanonicalStoreError::Configuration(_)
        | CanonicalStoreError::Validation(_)
        | CanonicalStoreError::Migration { .. }
        | CanonicalStoreError::MigrationChecksumMismatch { .. }
        | CanonicalStoreError::PrimaryWrite { .. }
        | CanonicalStoreError::VersionConflict { .. }
        | CanonicalStoreError::IdempotencyConflict
        | CanonicalStoreError::IntegrityViolation(_) => {
            AgentJobError::terminal("AGENT_CANONICAL_INPUT_INVALID")
        }
    }
}

#[derive(Debug)]
struct Coc7AgentGameplayRules {
    repository: trpg_data_eventing::persistence_postgresql::CoreDomainRepository,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentNpcInteractionArguments {
    session_id: String,
    character_id: String,
    npc_id: String,
    approach: String,
    public_response: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentCombatRoundArguments {
    session_id: String,
    character_id: String,
    npc_id: String,
    action_kind: String,
    defense: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentChaseSegmentArguments {
    session_id: String,
    character_id: String,
    npc_id: String,
    initial_range: i8,
    obstacle_id: Option<String>,
    obstacle_cost: u8,
}

impl trpg_agent_runtime::agent_job::AgentGameplayRulePort for Coc7AgentGameplayRules {
    fn resolve<'a>(
        &'a self,
        job: &'a DurableAgentJob,
        call: &'a trpg_agent_runtime::agent_job::AgentJobToolCall,
    ) -> trpg_agent_runtime::agent_job::AgentGameplayRuleFuture<'a> {
        Box::pin(async move {
            use trpg_data_eventing::persistence_postgresql::PublicGameplayContextKind;
            use trpg_ruleset_coc7::coc7_rules_engine::{
                resolve_public_gameplay, PublicGameplayAction, PublicGameplayContext,
            };

            let (session_id, character_id, npc_id, kind, action) = match call.name.as_str() {
                "resolve_npc_interaction" => {
                    let value: AgentNpcInteractionArguments =
                        serde_json::from_value(call.arguments.clone())
                            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
                    let action = PublicGameplayAction::NpcInteraction {
                        character_id: value.character_id.clone(),
                        npc_id: value.npc_id.clone(),
                        approach: value.approach,
                        public_response: value.public_response,
                    };
                    (
                        value.session_id,
                        value.character_id,
                        value.npc_id,
                        PublicGameplayContextKind::NpcInteraction,
                        action,
                    )
                }
                "resolve_combat_round" => {
                    let value: AgentCombatRoundArguments =
                        serde_json::from_value(call.arguments.clone())
                            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
                    let action = PublicGameplayAction::CombatRound {
                        character_id: value.character_id.clone(),
                        npc_id: value.npc_id.clone(),
                        action_kind: value.action_kind,
                        defense: value.defense,
                    };
                    (
                        value.session_id,
                        value.character_id,
                        value.npc_id,
                        PublicGameplayContextKind::CombatRound,
                        action,
                    )
                }
                "resolve_chase_segment" => {
                    let value: AgentChaseSegmentArguments =
                        serde_json::from_value(call.arguments.clone())
                            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
                    let action = PublicGameplayAction::ChaseSegment {
                        character_id: value.character_id.clone(),
                        npc_id: value.npc_id.clone(),
                        initial_range: value.initial_range,
                        obstacle_id: value.obstacle_id,
                        obstacle_cost: value.obstacle_cost,
                    };
                    (
                        value.session_id,
                        value.character_id,
                        value.npc_id,
                        PublicGameplayContextKind::ChaseSegment {
                            initial_range: value.initial_range,
                        },
                        action,
                    )
                }
                _ => return Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED")),
            };
            let loaded = self
                .repository
                .load_public_gameplay_profile_context(
                    &job.campaign_id,
                    &session_id,
                    &character_id,
                    &npc_id,
                    kind,
                )
                .await
                .map_err(map_agent_gameplay_repository_error)?;
            let context = PublicGameplayContext {
                npc_public_identity: loaded.npc_public_identity,
                character_combat_profile: loaded.character_combat_profile,
                npc_combat_profile: loaded.npc_combat_profile,
                character_chase_profile: loaded.character_chase_profile,
                npc_chase_profile: loaded.npc_chase_profile,
            };
            let resolution = resolve_public_gameplay(&context, &action)
                .map_err(|_| AgentJobError::terminal("AGENT_GAMEPLAY_RULES_REJECTED"))?;
            serde_json::to_value(resolution)
                .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))
        })
    }
}

fn map_agent_gameplay_repository_error(
    error: trpg_data_eventing::persistence_postgresql::CoreDomainRepositoryError,
) -> AgentJobError {
    use trpg_data_eventing::persistence_postgresql::CoreDomainRepositoryError;
    match error {
        CoreDomainRepositoryError::Database(_) | CoreDomainRepositoryError::Canonical(_) => {
            AgentJobError::retryable("AGENT_GAMEPLAY_CONTEXT_UNAVAILABLE")
        }
        CoreDomainRepositoryError::NotFound(_) => {
            AgentJobError::terminal("AGENT_GAMEPLAY_CONTEXT_NOT_FOUND")
        }
        CoreDomainRepositoryError::InvalidInput(_) | CoreDomainRepositoryError::Forbidden => {
            AgentJobError::terminal("AGENT_GAMEPLAY_CONTEXT_REJECTED")
        }
        CoreDomainRepositoryError::Domain(_)
        | CoreDomainRepositoryError::Serialization
        | CoreDomainRepositoryError::PolicyEvidenceMismatch
        | CoreDomainRepositoryError::Integrity(_)
        | CoreDomainRepositoryError::ConcurrentStart => {
            AgentJobError::terminal("AGENT_GAMEPLAY_CONTEXT_INVALID")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn optional_agent_job_worker_from_environment(
    workflow: DurableWorkflowStore,
    runtime: &tokio::runtime::Runtime,
    provider: Option<HttpModelProvider<MountedFileSecretResolver>>,
    read_canonical: PostgresCanonicalStore,
    commit_canonical: Option<PostgresCanonicalStore>,
    secret_manager: &Arc<SecretManager<MountedFileSecretResolver>>,
    database_url: &SecretValue,
    redis_url: &SecretValue,
    witness_url: &SecretValue,
    worker_id: &str,
    redis_ca: Option<&[u8]>,
    redis_client_certificate: Option<&[u8]>,
    redis_client_private_key: Option<&[u8]>,
) -> Result<Option<AgentJobWorker>, String> {
    let Some(provider) = provider else {
        return Ok(None);
    };
    let commit_canonical =
        commit_canonical.ok_or_else(|| "AGENT_CANONICAL_STORE_REQUIRED".to_owned())?;
    let canonical_repository = read_canonical;
    let gameplay_repository = database_url
        .expose_utf8_to(|database| {
            runtime
                .block_on(trpg_data_eventing::persistence_postgresql::CoreDomainRepository::connect(
                    database,
                    canonical_repository.clone(),
                ))
                .map_err(|_| "AGENT_GAMEPLAY_DATABASE_CONNECTION_FAILED".to_owned())
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())??;
    let canonical_runtime = Arc::new(Mutex::new(
        tokio::runtime::Runtime::new()
            .map_err(|_| "AGENT_CANONICAL_RUNTIME_INITIALIZATION_FAILED".to_owned())?,
    ));
    let canonical = Arc::new(PostgresCanonicalCommitPort::new(
        canonical_runtime,
        commit_canonical,
    ));
    let (policy, audit) = agent_policy_and_audit_from_environment(secret_manager)?;
    let identity_signing_key = resolve_mounted_secret(secret_manager, "TRPG_IDENTITY_SIGNING_KEY")?
        .to_key32()
        .map_err(|_| "IDENTITY_SIGNING_KEY_INVALID".to_owned())?;
    let postgres_ca = optional_file_bytes("TRPG_POSTGRES_CA_CERT_PATH")?;
    let redis_namespace = std::env::var("TRPG_IDENTITY_REDIS_NAMESPACE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "trpg:identity".to_owned());
    let session_ttl_ms = bounded_environment_u64(
        "TRPG_IDENTITY_SESSION_TTL_MS",
        86_400_000,
        1_000,
        604_800_000,
    )?;
    let internal_credential_ttl_ms = bounded_environment_u64(
        "TRPG_AGENT_INTERNAL_CREDENTIAL_TTL_MS",
        60_000,
        1_000,
        300_000,
    )?;
    let argon2_concurrency = usize::try_from(bounded_environment_u64(
        "TRPG_IDENTITY_ARGON2_CONCURRENCY",
        4,
        1,
        64,
    )?)
    .map_err(|_| "TRPG_IDENTITY_ARGON2_CONCURRENCY_INVALID".to_owned())?;
    let mut decision_port = None;
    database_url
        .expose_utf8_to(|database| {
            redis_url.expose_utf8_to(|redis| {
                identity_signing_key.expose_to(|signing_key| {
                    decision_port = Some(GovernedAgentDecisionPort::from_prepared_postgres(
                        ProductionAgentIdentityConfiguration {
                            database_url: database,
                            postgres_ca_certificate_pem: postgres_ca.as_deref(),
                            redis_url: redis,
                            redis_namespace: &redis_namespace,
                            signing_key,
                            session_ttl_ms,
                            argon2_concurrency,
                            redis_root_certificate: redis_ca,
                            redis_client_certificate,
                            redis_client_private_key,
                            workload_id: worker_id,
                            internal_credential_ttl_ms,
                        },
                        policy,
                        audit,
                        canonical,
                    ));
                });
            })
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?
        .map_err(|_| "REDIS_URL_SECRET_INVALID".to_owned())?;
    let decision_port = decision_port
        .ok_or_else(|| "AGENT_DECISION_PORT_CONSTRUCTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|error| error.code().to_owned())?;
    let certification =
        local_model_certification_from_environment(&provider, secret_manager, witness_url)?;
    let configuration = AgentJobExecutionConfig {
        claim_owner: worker_id.to_owned(),
        lease_duration: Duration::from_millis(bounded_environment_u64(
            "TRPG_AGENT_JOB_LEASE_MS",
            30_000,
            1_000,
            300_000,
        )?),
        heartbeat_interval: Duration::from_millis(bounded_environment_u64(
            "TRPG_AGENT_JOB_HEARTBEAT_MS",
            5_000,
            100,
            60_000,
        )?),
        max_attempts: i32::try_from(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_ATTEMPTS",
            5,
            1,
            20,
        )?)
        .map_err(|_| "TRPG_AGENT_JOB_MAX_ATTEMPTS_INVALID".to_owned())?,
        max_context_bytes: usize::try_from(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_CONTEXT_BYTES",
            262_144,
            1_024,
            1_048_576,
        )?)
        .map_err(|_| "TRPG_AGENT_JOB_MAX_CONTEXT_BYTES_INVALID".to_owned())?,
        max_input_tokens: bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_INPUT_TOKENS",
            32_768,
            1,
            1_000_000,
        )?,
        max_output_tokens: bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_OUTPUT_TOKENS",
            4_096,
            1,
            100_000,
        )?,
        max_tool_calls: usize::try_from(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_TOOL_CALLS",
            1,
            1,
            1,
        )?)
        .map_err(|_| "TRPG_AGENT_JOB_MAX_TOOL_CALLS_INVALID".to_owned())?,
        max_tool_loops: usize::try_from(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_TOOL_LOOPS",
            1,
            1,
            1,
        )?)
        .map_err(|_| "TRPG_AGENT_JOB_MAX_TOOL_LOOPS_INVALID".to_owned())?,
    };
    let provider: Arc<dyn ExecutableModelProvider> = Arc::new(provider);
    let repository: Arc<dyn AgentJobRepository> = Arc::new(CanonicalAgentJobRepository::new(
        workflow.clone(),
        canonical_repository,
    ));
    let decisions: Arc<dyn trpg_agent_runtime::agent_job::AgentJobDecisionPort> =
        Arc::new(decision_port);
    let tools: Arc<dyn trpg_agent_runtime::agent_job::AgentJobToolPort> =
        Arc::new(GovernedAgentJobToolPort::with_gameplay(
            workflow.clone(),
            Arc::clone(&repository),
            Arc::new(Coc7AgentSkillCheckRules),
            Arc::new(Coc7AgentGameplayRules {
                repository: gameplay_repository,
            }),
        ));
    AgentJobWorker::new(
        repository,
        provider,
        tools,
        decisions,
        certification,
        configuration,
    )
    .map(Some)
    .map_err(|error| error.code().to_owned())
}
