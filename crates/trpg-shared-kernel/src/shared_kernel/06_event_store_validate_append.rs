
impl<P: Clone + PartialEq + Serialize> EventStore<P> {
    /// Performs every deterministic append guard without mutating the store.
    /// Callers that must persist an idempotent side effect before the event can
    /// use this while holding their exclusive `&mut EventStore` borrow; the
    /// subsequent append cannot encounter a stale version introduced by a
    /// concurrent in-process writer.
    pub fn validate_append<T>(
        &self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: &P,
    ) -> KernelResult<()> {
        validate_command_envelope(command)?;

        let campaign_id = command
            .authenticated_context()
            .resource()
            .campaign_id()
            .clone();
        let stream_id = command
            .authenticated_context()
            .resource()
            .resource_id()
            .clone();
        let idempotency_scope = (
            campaign_id.clone(),
            stream_id.clone(),
            command.idempotency_key.clone(),
        );
        let request_hash = append_request_hash(command, event_type, payload)?;
        if let Some(existing) = self.idempotency_index.get(&idempotency_scope) {
            return if existing.request_hash == request_hash {
                Ok(())
            } else {
                Err(TrpgError::DuplicateCommand)
            };
        }

        let actual_version = self.current_stream_version(&campaign_id, &stream_id);
        if command.expected_version != actual_version {
            return Err(TrpgError::ExpectedVersionConflict {
                expected: command.expected_version,
                actual: actual_version,
            });
        }
        Ok(())
    }

    pub fn append<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: P,
    ) -> KernelResult<EventEnvelope<P>> {
        self.validate_append(command, event_type, &payload)?;

        let campaign_id = command
            .authenticated_context()
            .resource()
            .campaign_id()
            .clone();
        let stream_id = command
            .authenticated_context()
            .resource()
            .resource_id()
            .clone();
        let idempotency_scope = (
            campaign_id.clone(),
            stream_id.clone(),
            command.idempotency_key.clone(),
        );
        let request_hash = append_request_hash(command, event_type, &payload)?;
        if let Some(existing) = self.idempotency_index.get(&idempotency_scope) {
            if existing.request_hash == request_hash {
                return Ok(self.events[existing.event_index].clone());
            }
            return Err(TrpgError::DuplicateCommand);
        }

        let actual_version = self.current_stream_version(&campaign_id, &stream_id);

        let mut event = EventEnvelope {
            sequence: self
                .events
                .iter()
                .map(|event| event.sequence)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(TrpgError::AuditIntegrityViolation)?,
            stream_id,
            stream_version: actual_version + 1,
            event_type,
            campaign_id,
            authenticated_actor: command.actor.clone(),
            resource: command.authenticated_context().resource().clone(),
            authority_contract_id: command
                .authenticated_context()
                .authority()
                .contract_id()
                .clone(),
            authority_owner: command
                .authenticated_context()
                .authority()
                .authority_owner()
                .clone(),
            command_id: command.command_id.clone(),
            idempotency_key: command.idempotency_key.clone(),
            authority_contract_version: command.authority_contract_version,
            visibility: command.visibility.clone(),
            fact_provenance: command.fact_provenance.clone(),
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.authenticated_context().trace_id().clone(),
            occurred_at_unix_ms: unix_time_ms(),
            payload: payload.clone(),
            recorded_payload: payload,
            integrity_hash: [0_u8; 32],
        };
        event.integrity_hash = event_integrity_hash(&event)?;

        self.idempotency_index.insert(
            idempotency_scope,
            IdempotencyRecord {
                request_hash,
                event_index: self.events.len(),
            },
        );
        self.events.push(event.clone());

        Ok(event)
    }

    /// Materializes a canonical event using only the identity returned by the
    /// durable adapter. This closes cold-restart sequence/timestamp forgery.
    pub fn record_canonical<T>(
        &mut self,
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        payload: P,
        durable: &CanonicalCommittedEvent,
    ) -> KernelResult<EventEnvelope<P>> {
        validate_command_envelope(command)?;
        if durable.sequence == 0
            || durable.occurred_at_unix_ms == 0
            || durable.stream_version
                != command
                    .expected_version
                    .checked_add(1)
                    .ok_or(TrpgError::AuditIntegrityViolation)?
            || durable.event_type != event_type
            || durable.command_id.trim().is_empty()
            || durable.idempotency_key.trim().is_empty()
            || !is_canonical_hmac(&durable.event_integrity_hash)
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        let local_payload =
            serde_json::to_value(&payload).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let durable_payload: serde_json::Value = serde_json::from_str(&durable.payload_json)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        if local_payload != durable_payload {
            return Err(TrpgError::AuditIntegrityViolation);
        }

        let campaign_id = command
            .authenticated_context()
            .resource()
            .campaign_id()
            .clone();
        let stream_id = command
            .authenticated_context()
            .resource()
            .resource_id()
            .clone();
        let idempotency_scope = (
            campaign_id.clone(),
            stream_id.clone(),
            command.idempotency_key.clone(),
        );
        let request_hash = append_request_hash(command, event_type, &payload)?;
        if let Some(existing) = self.idempotency_index.get(&idempotency_scope) {
            let event = &self.events[existing.event_index];
            if existing.request_hash == request_hash
                && event.sequence == durable.sequence
                && event.stream_version == durable.stream_version
                && event.command_id.as_str() == durable.command_id
                && event.idempotency_key == durable.idempotency_key
                && event.occurred_at_unix_ms == durable.occurred_at_unix_ms
            {
                return Ok(event.clone());
            }
            return Err(TrpgError::DuplicateCommand);
        }
        if self.events.iter().any(|event| {
            event.sequence == durable.sequence
                || (event.campaign_id == campaign_id
                    && event.stream_id == stream_id
                    && event.stream_version == durable.stream_version)
        }) {
            return Err(TrpgError::AuditIntegrityViolation);
        }

        let mut event = EventEnvelope {
            sequence: durable.sequence,
            stream_id: stream_id.clone(),
            stream_version: durable.stream_version,
            event_type,
            campaign_id: campaign_id.clone(),
            authenticated_actor: command.actor.clone(),
            resource: command.authenticated_context().resource().clone(),
            authority_contract_id: command
                .authenticated_context()
                .authority()
                .contract_id()
                .clone(),
            authority_owner: command
                .authenticated_context()
                .authority()
                .authority_owner()
                .clone(),
            command_id: EntityId::new(durable.command_id.clone())?,
            idempotency_key: durable.idempotency_key.clone(),
            authority_contract_version: command.authority_contract_version,
            visibility: command.visibility.clone(),
            fact_provenance: command.fact_provenance.clone(),
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.authenticated_context().trace_id().clone(),
            occurred_at_unix_ms: durable.occurred_at_unix_ms,
            payload: payload.clone(),
            recorded_payload: payload,
            integrity_hash: [0_u8; 32],
        };
        event.integrity_hash = event_integrity_hash(&event)?;
        self.idempotency_index.insert(
            idempotency_scope,
            IdempotencyRecord {
                request_hash,
                event_index: self.events.len(),
            },
        );
        self.events.push(event.clone());
        self.stream_base_versions
            .entry((campaign_id, stream_id))
            .and_modify(|version| *version = (*version).max(durable.stream_version))
            .or_insert(durable.stream_version);
        Ok(event)
    }
}

fn is_canonical_hmac(value: &str) -> bool {
    const PREFIX: &str = "hmac-sha256:";
    value.len() == PREFIX.len() + 64
        && value.starts_with(PREFIX)
        && value[PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

impl<P> EventStore<P> {
    pub fn with_stream_base_version_for(
        campaign_id: EntityId,
        stream_id: EntityId,
        stream_base_version: u64,
    ) -> Self {
        let mut store = Self::default();
        store
            .stream_base_versions
            .insert((campaign_id, stream_id), stream_base_version);
        store
    }

    pub fn events(&self) -> &[EventEnvelope<P>] {
        &self.events
    }

    pub fn current_stream_version(&self, campaign_id: &EntityId, stream_id: &EntityId) -> u64 {
        let seeded = self
            .stream_base_versions
            .get(&(campaign_id.clone(), stream_id.clone()))
            .copied()
            .unwrap_or(0);
        self.events
            .iter()
            .filter(|event| &event.campaign_id == campaign_id && &event.stream_id == stream_id)
            .map(|event| event.stream_version)
            .max()
            .unwrap_or(seeded)
            .max(seeded)
    }
}

fn append_request_hash<T, P: Clone + Serialize>(
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    payload: &P,
) -> KernelResult<[u8; 32]> {
    let mut proposed = EventEnvelope {
        sequence: 0,
        stream_id: command
            .authenticated_context()
            .resource()
            .resource_id()
            .clone(),
        stream_version: command.expected_version.saturating_add(1),
        event_type,
        campaign_id: command
            .authenticated_context()
            .resource()
            .campaign_id()
            .clone(),
        authenticated_actor: command.actor.clone(),
        resource: command.authenticated_context().resource().clone(),
        authority_contract_id: command
            .authenticated_context()
            .authority()
            .contract_id()
            .clone(),
        authority_owner: command
            .authenticated_context()
            .authority()
            .authority_owner()
            .clone(),
        command_id: command.command_id.clone(),
        idempotency_key: command.idempotency_key.clone(),
        authority_contract_version: command.authority_contract_version,
        visibility: command.visibility.clone(),
        fact_provenance: command.fact_provenance.clone(),
        correlation_id: command.correlation_id.clone(),
        causation_id: command.causation_id.clone(),
        trace_id: command.authenticated_context().trace_id().clone(),
        occurred_at_unix_ms: 0,
        payload: payload.clone(),
        recorded_payload: payload.clone(),
        integrity_hash: [0_u8; 32],
    };
    proposed.integrity_hash = event_integrity_hash(&proposed)?;
    Ok(proposed.integrity_hash)
}
