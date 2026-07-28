
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

fn restricted_visibility(label: &VisibilityLabel) -> bool {
    label.is_restricted()
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

fn stable_projection_hash(events: &[OpsEventEnvelope]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for event in events {
        update_hash(&mut hash, event.sequence.to_string().as_bytes());
        update_hash(&mut hash, event.event_type.as_bytes());
        if let OpsEvent::RunbookStepRecorded(record) = &event.payload {
            update_hash(&mut hash, record.module_name.as_bytes());
            update_hash(&mut hash, record.operation.as_str().as_bytes());
        }
    }

    format!("sha256:ops-{hash:016x}")
}

fn update_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}
