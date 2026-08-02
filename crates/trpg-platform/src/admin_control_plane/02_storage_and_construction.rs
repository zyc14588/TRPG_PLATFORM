impl AdminControlPlane {
    pub fn from_environment(
        operations: Arc<dyn AdminOperations>,
    ) -> Result<Self, AdminControlPlaneError> {
        let secret_mount = required_environment("TRPG_SECRET_MOUNT")?;
        let catalog_path = required_environment("TRPG_SECRET_CATALOG_PATH")?;
        let resolver = MountedFileSecretResolver::new(secret_mount)
            .map_err(|_| AdminControlPlaneError::Configuration("ADMIN_SECRET_MOUNT_INVALID"))?;
        let secret_manager = Arc::new(
            SecretManager::new_durable(resolver, catalog_path).map_err(|_| {
                AdminControlPlaneError::Configuration("ADMIN_SECRET_CATALOG_INVALID")
            })?,
        );
        let database_url = resolve_mounted_secret(&secret_manager, "TRPG_DATABASE_URL")?;
        let redis_url = resolve_mounted_secret(&secret_manager, "TRPG_REDIS_URL")?;
        let signing_key = resolve_mounted_secret(&secret_manager, "TRPG_IDENTITY_SIGNING_KEY")?
            .to_key32()
            .map_err(|_| {
                AdminControlPlaneError::Configuration("ADMIN_IDENTITY_SIGNING_KEY_INVALID")
            })?;
        let bootstrap_token =
            resolve_mounted_secret(&secret_manager, "TRPG_ADMIN_BOOTSTRAP_TOKEN")?;
        validate_bootstrap_token(&bootstrap_token)?;
        let audit_key = resolve_mounted_secret(&secret_manager, "TRPG_AUDIT_HMAC_KEY")?
            .to_key32()
            .map_err(|_| {
                AdminControlPlaneError::Configuration("ADMIN_AUDIT_HMAC_KEY_INVALID")
            })?;
        let postgres_ca = optional_regular_file("TRPG_POSTGRES_CA_CERT_PATH")?;
        let redis_ca = optional_regular_file("TRPG_REDIS_CA_CERT_PATH")?;
        let redis_client_certificate =
            optional_regular_file("TRPG_REDIS_CLIENT_CERT_PATH")?;
        let redis_client_private_key =
            optional_regular_file("TRPG_REDIS_CLIENT_KEY_PATH")?;
        let redis_namespace = optional_environment("TRPG_REDIS_LOGIN_NAMESPACE")
            .unwrap_or_else(|| "trpg:identity".to_owned());
        let session_ttl_ms = bounded_environment_u64(
            "TRPG_IDENTITY_SESSION_TTL_MS",
            8 * 60 * 60 * 1_000,
            60_000,
            7 * 24 * 60 * 60 * 1_000,
        )?;
        let argon2_concurrency = usize::try_from(bounded_environment_u64(
            "TRPG_ARGON2_MAX_CONCURRENCY",
            2,
            1,
            64,
        )?)
        .map_err(|_| AdminControlPlaneError::Configuration("ADMIN_ARGON2_LIMIT_INVALID"))?;
        let local_provider_network_policy = LocalProviderNetworkPolicy::parse(
            &optional_environment("TRPG_LOCAL_PROVIDER_ENDPOINT_ALLOWLIST")
                .unwrap_or_else(|| "loopback".to_owned()),
        )
        .map_err(|_| {
            AdminControlPlaneError::Configuration("ADMIN_LOCAL_PROVIDER_ALLOWLIST_INVALID")
        })?;

        let mut identity_result = None;
        database_url
            .expose_utf8_to(|database| {
                redis_url.expose_utf8_to(|redis| {
                    signing_key.expose_to(|key| {
                        identity_result = Some(
                            IdentityService::from_prepared_postgres_with_security_and_redis_tls(
                                database,
                                postgres_ca.as_deref(),
                                redis,
                                &redis_namespace,
                                key,
                                session_ttl_ms,
                                argon2_concurrency,
                                redis_ca.as_deref(),
                                redis_client_certificate.as_deref(),
                                redis_client_private_key.as_deref(),
                            ),
                        );
                    });
                })
            })
            .map_err(|_| {
                AdminControlPlaneError::Configuration("ADMIN_DATABASE_URL_SECRET_INVALID")
            })?
            .map_err(|_| {
                AdminControlPlaneError::Configuration("ADMIN_REDIS_URL_SECRET_INVALID")
            })?;
        let identity = identity_result
            .ok_or(AdminControlPlaneError::Configuration(
                "ADMIN_IDENTITY_CONSTRUCTION_NOT_ATTEMPTED",
            ))?
            .map_err(|_| {
                AdminControlPlaneError::Configuration("ADMIN_IDENTITY_INITIALIZATION_FAILED")
            })?;
        let state_path = PathBuf::from(required_environment("TRPG_ADMIN_STATE_PATH")?);
        prepare_private_parent(&state_path)?;
        let audit_path = PathBuf::from(required_environment("TRPG_ADMIN_AUDIT_LOG_PATH")?);
        prepare_private_parent(&audit_path)?;
        let audit_key_id = required_environment("TRPG_AUDIT_HMAC_KEY_ID")?;
        let mut audit_result = None;
        audit_key.expose_to(|key| {
            audit_result = Some(FileAuditLog::open(&audit_path, audit_key_id, key));
        });
        let audit = audit_result
            .ok_or(AdminControlPlaneError::AuditIntegrity)?
            .map_err(|_| AdminControlPlaneError::AuditIntegrity)?;
        let control = Self {
            state_path,
            bootstrap_token,
            identity,
            secret_manager,
            audit,
            operations,
            local_provider_network_policy,
        };
        control.load_state()?;
        Ok(control)
    }

    pub fn readiness(&mut self) -> Result<String, AdminControlPlaneError> {
        self.identity
            .check_readiness()
            .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_IDENTITY_NOT_READY"))?;
        let state = self.load_state()?;
        self.audit
            .verify()
            .map_err(|_| AdminControlPlaneError::AuditIntegrity)?;
        Ok(format!(
            "admin control ready; bootstrap_completed={}; state_version={}",
            state.bootstrap_token_consumed, state.version
        ))
    }

    fn load_state(&self) -> Result<AdminState, AdminControlPlaneError> {
        let _lock = StateFileLock::acquire(&self.state_path)?;
        read_state_unlocked(&self.state_path)
    }

}

fn read_state_unlocked(path: &Path) -> Result<AdminState, AdminControlPlaneError> {
    if !path.exists() {
        let state = AdminState::default();
        write_state_unlocked(path, &state)?;
        return Ok(state);
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_METADATA_FAILED"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AdminControlPlaneError::Persistence(
            "ADMIN_STATE_REGULAR_FILE_REQUIRED",
        ));
    }
    let bytes = fs::read(path)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_READ_FAILED"))?;
    let state: AdminState = serde_json::from_slice(&bytes)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_INVALID"))?;
    if state.schema_version != ADMIN_STATE_SCHEMA || state.receipts.len() > MAX_RECEIPTS {
        return Err(AdminControlPlaneError::Persistence(
            "ADMIN_STATE_SCHEMA_INVALID",
        ));
    }
    Ok(state)
}

fn write_state_unlocked(
    path: &Path,
    state: &AdminState,
) -> Result<(), AdminControlPlaneError> {
    if state.schema_version != ADMIN_STATE_SCHEMA || state.receipts.len() > MAX_RECEIPTS {
        return Err(AdminControlPlaneError::Persistence(
            "ADMIN_STATE_SCHEMA_INVALID",
        ));
    }
    let encoded = serde_json::to_vec_pretty(state)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_ENCODE_FAILED"))?;
    let timestamp = now_unix_ms()?;
    let temporary = path.with_extension(format!("tmp.{}.{}", std::process::id(), timestamp));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_CREATE_FAILED"))?;
    use std::io::Write as _;
    file.write_all(&encoded)
        .and_then(|()| file.sync_all())
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_WRITE_FAILED"))?;
    set_private_permissions(&temporary, 0o600)?;
    fs::rename(&temporary, path)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_PUBLISH_FAILED"))?;
    sync_parent(path)
}

struct StateFileLock {
    path: PathBuf,
}

impl StateFileLock {
    fn acquire(state_path: &Path) -> Result<Self, AdminControlPlaneError> {
        let lock_path = state_path.with_extension("lock");
        for _ in 0..LOCK_RETRIES {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut file) => {
                    use std::io::Write as _;
                    write!(file, "{}", std::process::id()).map_err(|_| {
                        AdminControlPlaneError::Persistence("ADMIN_STATE_LOCK_WRITE_FAILED")
                    })?;
                    file.sync_all().map_err(|_| {
                        AdminControlPlaneError::Persistence("ADMIN_STATE_LOCK_WRITE_FAILED")
                    })?;
                    set_private_permissions(&lock_path, 0o600)?;
                    return Ok(Self { path: lock_path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if stale_process_lock(&lock_path)? {
                        let _ = fs::remove_file(&lock_path);
                    } else {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                Err(_) => {
                    return Err(AdminControlPlaneError::Persistence(
                        "ADMIN_STATE_LOCK_FAILED",
                    ))
                }
            }
        }
        Err(AdminControlPlaneError::Persistence(
            "ADMIN_STATE_LOCK_TIMEOUT",
        ))
    }
}

impl Drop for StateFileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn stale_process_lock(path: &Path) -> Result<bool, AdminControlPlaneError> {
    let value = fs::read_to_string(path)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_LOCK_INVALID"))?;
    let pid = value
        .parse::<u32>()
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_LOCK_INVALID"))?;
    Ok(!Path::new("/proc").join(pid.to_string()).exists())
}
