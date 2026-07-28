
pub fn is_current_safe_name(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let denied = [
        "generated-from-source",
        "generated_from_source",
        "source-breakdow",
        "source_breakdow",
        "docs-implementation",
        "docs_implementation",
        "implementation-90",
        "implementation_90",
        "fix-history",
        "fix_history",
        "legacy",
        "v3",
        "v4",
        "v5",
        "v6",
    ];

    if denied.iter().any(|token| lower.contains(token)) {
        return false;
    }

    trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        && !has_long_hex_run(trimmed)
}

fn has_long_hex_run(value: &str) -> bool {
    let mut run = 0;
    for ch in value.chars() {
        if ch.is_ascii_hexdigit() {
            run += 1;
            if run >= 10 {
                return true;
            }
        } else {
            run = 0;
        }
    }

    false
}

fn projection_hash_input(events: &[EventEnvelope<DataEventPayload>]) -> Vec<u8> {
    let mut input = Vec::new();
    append_projection_hash_field(&mut input, 1, b"trpg-data-event-projection-v1");
    append_projection_hash_field(&mut input, 2, &(events.len() as u64).to_be_bytes());
    for event in events {
        append_projection_hash_field(&mut input, 3, &event.sequence.to_be_bytes());
        append_projection_hash_field(&mut input, 4, event.stream_id.as_str().as_bytes());
        append_projection_hash_field(&mut input, 5, &event.stream_version.to_be_bytes());
        append_projection_hash_field(&mut input, 6, event.event_type.as_bytes());
        append_projection_hash_field(&mut input, 7, event.campaign_id.as_str().as_bytes());
        append_projection_hash_field(
            &mut input,
            8,
            event.authenticated_actor.id().as_str().as_bytes(),
        );
        append_projection_hash_field(
            &mut input,
            9,
            actor_role_name(event.authenticated_actor.role()).as_bytes(),
        );
        append_actor_origin_hash_fields(&mut input, event.authenticated_actor.origin());
        append_projection_hash_field(
            &mut input,
            14,
            event.resource.campaign_id().as_str().as_bytes(),
        );
        append_projection_hash_field(
            &mut input,
            15,
            event.resource.resource_type().as_str().as_bytes(),
        );
        append_projection_hash_field(
            &mut input,
            16,
            event.resource.resource_id().as_str().as_bytes(),
        );
        append_projection_hash_field(
            &mut input,
            17,
            event.authority_contract_id.as_str().as_bytes(),
        );
        append_projection_hash_field(&mut input, 18, event.authority_owner.as_str().as_bytes());
        append_projection_hash_field(&mut input, 19, event.command_id.as_str().as_bytes());
        append_projection_hash_field(&mut input, 20, event.idempotency_key.as_bytes());
        append_projection_hash_field(
            &mut input,
            21,
            &event.authority_contract_version.to_be_bytes(),
        );
        append_projection_hash_field(&mut input, 22, event.visibility.label().as_str().as_bytes());
        append_projection_hash_field(
            &mut input,
            23,
            event
                .visibility
                .subject_id()
                .map(EntityId::as_str)
                .unwrap_or_default()
                .as_bytes(),
        );
        append_projection_hash_field(
            &mut input,
            24,
            provenance_kind_name(&event.fact_provenance.kind).as_bytes(),
        );
        append_projection_hash_field(
            &mut input,
            25,
            event.fact_provenance.reference.as_str().as_bytes(),
        );
        append_projection_hash_field(
            &mut input,
            26,
            event.fact_provenance.recorded_by.as_str().as_bytes(),
        );
        append_projection_hash_field(&mut input, 27, event.correlation_id.as_str().as_bytes());
        append_projection_hash_field(&mut input, 28, event.causation_id.as_str().as_bytes());
        append_projection_hash_field(&mut input, 29, event.trace_id.as_str().as_bytes());
        append_projection_hash_field(&mut input, 30, event.payload.module_name.as_bytes());
        append_projection_hash_field(&mut input, 31, event.payload.event_name.as_bytes());
        append_projection_hash_field(&mut input, 32, event.payload.operation.as_str().as_bytes());
        append_projection_hash_field(
            &mut input,
            33,
            &(event.payload.read_models.len() as u64).to_be_bytes(),
        );
        for read_model in event.payload.read_models {
            append_projection_hash_field(&mut input, 34, read_model.as_bytes());
        }
    }
    input
}

fn append_projection_hash_field(input: &mut Vec<u8>, tag: u16, value: &[u8]) {
    input.extend_from_slice(&tag.to_be_bytes());
    input.extend_from_slice(&(value.len() as u64).to_be_bytes());
    input.extend_from_slice(value);
}

fn append_actor_origin_hash_fields(input: &mut Vec<u8>, origin: &ActorOrigin) {
    match origin {
        ActorOrigin::UserSession { session_id } => {
            append_projection_hash_field(input, 10, b"user_session");
            append_projection_hash_field(input, 11, session_id.as_str().as_bytes());
        }
        ActorOrigin::Workload { role } => {
            append_projection_hash_field(input, 10, b"workload");
            append_projection_hash_field(input, 11, workload_role_name(*role).as_bytes());
        }
        ActorOrigin::AgentRun {
            run_id,
            class,
            campaign_id,
        } => {
            append_projection_hash_field(input, 10, b"agent_run");
            append_projection_hash_field(input, 11, run_id.as_str().as_bytes());
            append_projection_hash_field(input, 12, agent_class_name(*class).as_bytes());
            append_projection_hash_field(input, 13, campaign_id.as_str().as_bytes());
        }
    }
}

fn actor_role_name(role: &ActorRole) -> &'static str {
    match role {
        ActorRole::ServerOwner => "server_owner",
        ActorRole::CampaignOwner => "campaign_owner",
        ActorRole::HumanKeeper => "human_keeper",
        ActorRole::AiKeeper => "ai_keeper",
        ActorRole::Investigator => "investigator",
        ActorRole::Moderator => "moderator",
        ActorRole::Spectator => "spectator",
        ActorRole::Workflow => "workflow",
        ActorRole::RulesEngine => "rules_engine",
        ActorRole::System => "system",
    }
}

fn workload_role_name(role: WorkloadRole) -> &'static str {
    match role {
        WorkloadRole::ApiServer => "api_server",
        WorkloadRole::RealtimeServer => "realtime_server",
        WorkloadRole::AgentWorker => "agent_worker",
        WorkloadRole::WorkflowEngine => "workflow_engine",
        WorkloadRole::RulesEngine => "rules_engine",
        WorkloadRole::AuditWriter => "audit_writer",
    }
}

fn agent_class_name(class: AgentClass) -> &'static str {
    match class {
        AgentClass::AiKeeperOrchestrator => "ai_keeper_orchestrator",
        AgentClass::KeeperCopilot => "keeper_copilot",
        AgentClass::AtmosphereWriter => "atmosphere_writer",
        AgentClass::MemoryCurator => "memory_curator",
    }
}

fn provenance_kind_name(kind: &ProvenanceKind) -> &'static str {
    match kind {
        ProvenanceKind::UserStatement => "user_statement",
        ProvenanceKind::HumanKeeperStatement => "human_keeper_statement",
        ProvenanceKind::RulesEngineDecision => "rules_engine_decision",
        ProvenanceKind::ToolResult => "tool_result",
        ProvenanceKind::AgentProposal => "agent_proposal",
        ProvenanceKind::ImportedSource => "imported_source",
        ProvenanceKind::SystemFixture => "system_fixture",
    }
}

fn current_unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn sha256_hex(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut words = [0u32; 64];
    for chunk in message.chunks_exact(64) {
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    state
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect::<String>()
}
