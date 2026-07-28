
impl<R: SecretResolver> SecretManager<R> {
    pub fn new(resolver: R) -> Self {
        Self {
            resolver,
            catalog: RwLock::new(SecretCatalog::default()),
            durable: None,
        }
    }

    pub fn new_durable(resolver: R, catalog_path: impl AsRef<Path>) -> KernelResult<Self> {
        let mut durable = DurableSecretCatalog::open(catalog_path.as_ref())?;
        let catalog = durable.load()?;
        Ok(Self {
            resolver,
            catalog: RwLock::new(catalog),
            durable: Some(Mutex::new(durable)),
        })
    }

    pub fn register(&self, reference: &SecretReference) -> KernelResult<()> {
        if self.durable.is_some() {
            let mutation = CatalogMutation::Register {
                backend: reference.backend,
                secret_id: reference.secret_id.clone(),
                version: reference.version,
            };
            return self.apply_durable(&mutation);
        }
        self.catalog
            .write()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .register(reference)
    }

    pub fn rotate(
        &self,
        current: &SecretReference,
        replacement: &SecretReference,
    ) -> KernelResult<()> {
        if self.durable.is_some() {
            let mutation = CatalogMutation::Rotate {
                backend: current.backend,
                secret_id: current.secret_id.clone(),
                current_version: current.version,
                replacement_version: replacement.version,
            };
            return self.apply_durable(&mutation);
        }
        self.catalog
            .write()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .rotate(current, replacement)
    }

    pub fn revoke(&self, reference: &SecretReference) -> KernelResult<()> {
        if self.durable.is_some() {
            let mutation = CatalogMutation::Revoke {
                backend: reference.backend,
                secret_id: reference.secret_id.clone(),
                version: reference.version,
            };
            return self.apply_durable(&mutation);
        }
        self.catalog
            .write()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .revoke(reference)
    }

    pub fn resolve(&self, reference: &SecretReference) -> KernelResult<SecretValue> {
        if let Some(durable) = &self.durable {
            let catalog = durable
                .lock()
                .map_err(|_| TrpgError::AuditIntegrityViolation)?
                .load()?;
            catalog.authorize(reference)?;
            *self
                .catalog
                .write()
                .map_err(|_| TrpgError::AuditIntegrityViolation)? = catalog;
            return self.resolver.resolve(reference);
        }
        self.catalog
            .read()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .authorize(reference)?;
        self.resolver.resolve(reference)
    }

    fn apply_durable(&self, mutation: &CatalogMutation) -> KernelResult<()> {
        let catalog = self
            .durable
            .as_ref()
            .expect("durable mutation requires durable catalog")
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .apply(mutation)?;
        *self
            .catalog
            .write()
            .map_err(|_| TrpgError::AuditIntegrityViolation)? = catalog;
        Ok(())
    }
}
