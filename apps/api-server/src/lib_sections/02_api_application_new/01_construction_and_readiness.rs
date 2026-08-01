
impl ApiApplication {
    pub fn new(identity: IdentityService) -> Self {
        let identity_verifier = identity.verifier();
        Self {
            authentication: AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            identity_verifier,
            membership_governance: None,
            canonical_custody: None,
        }
    }

    pub fn new_governed(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
    ) -> Self {
        let identity_verifier = identity.verifier();
        let audit = FormalCommitAudit::from_file_log(audit);
        Self {
            authentication: AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            identity_verifier,
            membership_governance: Some(Arc::new(Mutex::new(MembershipGovernance {
                policy,
                audit,
            }))),
            canonical_custody: None,
        }
    }

    pub fn new_production_governed(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
    ) -> Self {
        Self::new_production_governed_internal(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_production_governed_with_player_actions(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
        player_action_repository: CoreDomainRepository,
    ) -> Self {
        Self::new_production_governed_internal(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            Some(player_action_repository),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_production_governed_with_v1_lifecycle(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
        core_domain_database_url: &str,
    ) -> Result<Self, String> {
        let repository = canonical_runtime
            .block_on(CoreDomainRepository::connect(
                core_domain_database_url,
                canonical_store.clone(),
            ))
            .map_err(|_| "CORE_DOMAIN_DATABASE_CONNECTION_FAILED".to_owned())?;
        Ok(Self::new_production_governed_internal(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            Some(repository),
            None,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_production_governed_with_agent_jobs(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
        player_action_repository: Option<CoreDomainRepository>,
        workflow: DurableWorkflowStore,
        route: AgentJobRouteConfiguration,
    ) -> Result<Self, String> {
        route.validate()?;
        Ok(Self::new_production_governed_internal(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            player_action_repository,
            Some(AgentJobGateway { workflow, route }),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn new_production_governed_internal(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
        player_action_repository: Option<CoreDomainRepository>,
        agent_jobs: Option<AgentJobGateway>,
    ) -> Self {
        let identity_verifier = identity.verifier();
        let audit = FormalCommitAudit::from_file_log(audit);
        let runtime = Arc::new(Mutex::new(canonical_runtime));
        let canonical: Arc<dyn CanonicalCommitPort> = Arc::new(PostgresCanonicalCommitPort::new(
            Arc::clone(&runtime),
            canonical_store.clone(),
        ));
        let authorizer =
            FormalCommitAuthorizer::new(identity_verifier.clone(), policy.clone(), audit.clone());
        let export_storage_root = std::env::var_os("TRPG_EXPORT_STORAGE_ROOT")
            .map(PathBuf::from)
            .filter(|root| {
                root.is_absolute()
                    && root.parent().is_some()
                    && !std::fs::symlink_metadata(root)
                        .is_ok_and(|metadata| metadata.file_type().is_symlink())
            });
        Self {
            authentication: AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            identity_verifier,
            membership_governance: Some(Arc::new(Mutex::new(MembershipGovernance {
                policy,
                audit,
            }))),
            canonical_custody: Some(Arc::new(CanonicalCustody {
                runtime,
                privacy_runtime: Mutex::new(privacy_runtime),
                store: canonical_store,
                canonical: Arc::clone(&canonical),
                authorizer: authorizer.clone(),
                deletion_repository,
                runtime_events: trpg_runtime::EventStore::with_formal_custody(
                    authorizer.clone(),
                    Arc::clone(&canonical),
                ),
                agent_events: trpg_agent_runtime::AgentEventStore::with_formal_custody(
                    authorizer, canonical,
                ),
                lifecycle_port: player_action_repository
                    .clone()
                    .map(RepositoryCampaignCharacterPort::new),
                player_action_port: player_action_repository.map(RepositoryPlayerActionPort::new),
                agent_jobs,
                export_storage_root,
            })),
        }
    }

    pub fn readiness(&self) -> Result<String, String> {
        self.authentication
            .identity()
            .lock()
            .map_err(|_| "identity state lock poisoned".to_owned())?
            .check_readiness()
            .map_err(|error| error.code().to_owned())?;
        let governance = self
            .membership_governance
            .as_ref()
            .ok_or_else(|| "POLICY_UNAVAILABLE".to_owned())?;
        governance
            .lock()
            .map_err(|_| "policy state lock poisoned".to_owned())?
            .policy
            .check_readiness()
            .map_err(|error| error.code().to_owned())?;
        if let Some(custody) = &self.canonical_custody {
            custody.check_readiness()?;
        }
        Ok(
            "persistent identity, authorization, canonical event and witness state ready"
                .to_owned(),
        )
    }

}
