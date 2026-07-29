use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use trpg_agent_runtime::agent_runtime::AgentResult;
use trpg_agent_runtime::local_model_certification::{
    CertificationInput, LocalModelCertificate, LocalModelCertificationAuthority,
};

#[path = "certification_ledger_integrity/checkpoint_store.rs"]
mod checkpoint_store;
use checkpoint_store::TestFileCheckpointStore;
#[path = "certification_ledger_integrity/crash_boundaries.rs"]
mod crash_boundaries;

type HmacSha256 = Hmac<Sha256>;
static NEXT_DIR: AtomicU64 = AtomicU64::new(1);
const SIGNING_KEY: [u8; 32] = [0x5a; 32];
const CHILD_PATH: &str = "TRPG_CERTIFICATION_LEDGER_CHILD_PATH";
const CHILD_ID: &str = "TRPG_CERTIFICATION_LEDGER_CHILD_ID";
fn test_root() -> PathBuf {
    let suffix = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "certification-ledger-integrity-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    root
}
fn companion(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
fn open_authority(path: &Path) -> LocalModelCertificationAuthority {
    open_authority_result(path).unwrap()
}

fn open_authority_result(path: &Path) -> AgentResult<LocalModelCertificationAuthority> {
    LocalModelCertificationAuthority::new_with_checkpoint(
        "test-signing-key",
        &SIGNING_KEY,
        path,
        TestFileCheckpointStore::shared(companion(path, ".external-witness")),
    )
}

fn input(model_id: &str) -> CertificationInput {
    CertificationInput {
        model_id: model_id.to_owned(),
        json_schema_support: true,
        tool_call_support: true,
        visibility_tests_pass: true,
        prompt_injection_tests_pass: true,
        rules_eval_pass: true,
        latency_ms: 250,
    }
}

fn issue(authority: &LocalModelCertificationAuthority, model_id: &str) -> LocalModelCertificate {
    authority
        .issue_level4(
            &input(model_id),
            &format!("sha256:{}", "1".repeat(64)),
            "level4-suite",
            Duration::from_secs(60),
        )
        .unwrap()
}

fn populated_registry() -> (PathBuf, PathBuf, LocalModelCertificate) {
    let root = test_root();
    let path = root.join("certification-registry.jsonl");
    let authority = open_authority(&path);
    let revoked = issue(&authority, "model-one");
    issue(&authority, "model-two");
    authority.revoke(&revoked).unwrap();
    drop(authority);
    (root, path, revoked)
}

fn registry_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn write_lines(path: &Path, lines: &[String]) {
    fs::write(path, format!("{}\n", lines.join("\n"))).unwrap();
}

fn assert_tamper_rejected(mutate: impl FnOnce(&Path, &mut Vec<String>)) {
    let (root, path, _) = populated_registry();
    let mut lines = registry_lines(&path);
    mutate(&path, &mut lines);
    let rejected = open_authority_result(&path).is_err();
    fs::remove_dir_all(root).unwrap();
    assert!(rejected, "tampered certification registry must fail closed");
}

#[test]
fn certification_registry_rejects_revocation_tail_truncation() {
    assert_tamper_rejected(|path, lines| {
        lines.truncate(2);
        write_lines(path, lines);
    });
}

#[test]
fn certification_registry_rejects_reorder_copy_deletion_mutation_and_partial_line() {
    assert_tamper_rejected(|path, lines| {
        lines.swap(0, 1);
        write_lines(path, lines);
    });
    assert_tamper_rejected(|path, lines| {
        lines.insert(1, lines[0].clone());
        write_lines(path, lines);
    });
    assert_tamper_rejected(|path, lines| {
        lines.remove(1);
        write_lines(path, lines);
    });
    assert_tamper_rejected(|path, lines| {
        lines[0] = lines[0].replacen("model-one", "model-nine", 1);
        write_lines(path, lines);
    });
    assert_tamper_rejected(|path, _| {
        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(br#"{"schema_version":1"#)
            .unwrap();
    });
}

#[test]
fn certification_registry_rejects_combined_log_and_anchor_snapshot_rollback() {
    let root = test_root();
    let path = root.join("certification-registry.jsonl");
    let authority = open_authority(&path);
    let certificate = issue(&authority, "model-one");
    let old_log = fs::read(&path).unwrap();
    let anchor_path = companion(&path, ".head");
    let old_anchor = fs::read(&anchor_path).unwrap();

    authority.revoke(&certificate).unwrap();
    assert!(authority
        .ensure_ai_keeper_model(
            &certificate,
            certificate.model_id(),
            certificate.model_artifact_sha256()
        )
        .is_err());
    drop(authority);

    fs::write(&path, old_log).unwrap();
    fs::write(&anchor_path, old_anchor).unwrap();
    let reactivated = open_authority_result(&path)
        .and_then(|authority| {
            authority.ensure_ai_keeper_model(
                &certificate,
                certificate.model_id(),
                certificate.model_artifact_sha256(),
            )
        })
        .is_ok();
    fs::remove_dir_all(root).unwrap();

    assert!(
        !reactivated,
        "restoring the log and its sibling anchor must not reactivate a revoked certificate"
    );
}

#[test]
fn certification_registry_rejects_a_forged_external_checkpoint_mac() {
    let (root, path, _) = populated_registry();
    let witness_path = companion(&path, ".external-witness");
    let mut encoded = fs::read_to_string(&witness_path).unwrap().into_bytes();
    let mac = encoded
        .windows(b"hmac-sha256:".len())
        .rposition(|window| window == b"hmac-sha256:")
        .unwrap()
        + b"hmac-sha256:".len();
    encoded[mac] = if encoded[mac] == b'a' { b'b' } else { b'a' };
    fs::write(witness_path, encoded).unwrap();
    assert!(open_authority_result(&path).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn certification_registry_ignores_uncommitted_anchor_temporary_file() {
    let (root, path, _) = populated_registry();
    fs::write(companion(&path, ".head.tmp-crash"), b"uncommitted").unwrap();
    let reopened = open_authority_result(&path).is_ok();
    fs::remove_dir_all(root).unwrap();
    assert!(
        reopened,
        "a pre-rename temporary file is not committed state"
    );
}

fn previous_registry_line(line: &str) -> String {
    let record: Value = serde_json::from_str(line).unwrap();
    let certificate = record["certificate"].clone();
    let state = record["state"].as_str().unwrap();
    let mut mac = HmacSha256::new_from_slice(&SIGNING_KEY).unwrap();
    for field in [
        certificate["certificate_id"].as_str().unwrap(),
        certificate["signature"].as_str().unwrap(),
        if state == "Active" {
            "active"
        } else {
            "revoked"
        },
    ] {
        mac.update(&(field.len() as u64).to_be_bytes());
        mac.update(field.as_bytes());
    }
    json!({
        "certificate": certificate,
        "state": state,
        "registry_mac": format!("hmac-sha256:{:x}", mac.finalize().into_bytes())
    })
    .to_string()
}

#[test]
fn certification_previous_format_migration_is_explicit_auditable_and_one_time() {
    let (root, path, revoked) = populated_registry();
    let previous: Vec<_> = registry_lines(&path)
        .iter()
        .map(|line| previous_registry_line(line))
        .collect();
    let encoded = format!("{}\n", previous.join("\n"));
    fs::remove_file(companion(&path, ".head")).unwrap();
    fs::remove_file(companion(&path, ".external-witness")).unwrap();
    fs::write(&path, encoded.as_bytes()).unwrap();
    let digest = format!("sha256:{:x}", Sha256::digest(encoded.as_bytes()));

    assert!(open_authority_result(&path).is_err());
    assert!(
        LocalModelCertificationAuthority::migrate_previous_registry_with_checkpoint(
            "test-signing-key",
            &SIGNING_KEY,
            &path,
            previous.len() as u64,
            &format!("sha256:{}", "0".repeat(64)),
            TestFileCheckpointStore::shared(companion(&path, ".external-witness")),
        )
        .is_err()
    );
    LocalModelCertificationAuthority::migrate_previous_registry_with_checkpoint(
        "test-signing-key",
        &SIGNING_KEY,
        &path,
        previous.len() as u64,
        &digest,
        TestFileCheckpointStore::shared(companion(&path, ".external-witness")),
    )
    .unwrap();
    assert!(
        LocalModelCertificationAuthority::migrate_previous_registry_with_checkpoint(
            "test-signing-key",
            &SIGNING_KEY,
            &path,
            previous.len() as u64,
            &digest,
            TestFileCheckpointStore::shared(companion(&path, ".external-witness")),
        )
        .is_err()
    );

    let records: Vec<Value> = registry_lines(&path)
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(records
        .iter()
        .all(|record| record["source"] == "previous_format_migration"));
    let authority = open_authority(&path);
    assert!(authority
        .ensure_ai_keeper_model(
            &revoked,
            revoked.model_id(),
            revoked.model_artifact_sha256()
        )
        .is_err());
    drop(authority);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn certification_anchored_format_requires_controlled_checkpoint_migration() {
    let (root, path, revoked) = populated_registry();
    let encoded = fs::read(&path).unwrap();
    let digest = format!("sha256:{:x}", Sha256::digest(&encoded));
    fs::remove_file(companion(&path, ".external-witness")).unwrap();
    assert!(open_authority_result(&path).is_err());
    LocalModelCertificationAuthority::migrate_anchored_registry_checkpoint(
        "test-signing-key",
        &SIGNING_KEY,
        &path,
        registry_lines(&path).len() as u64,
        &digest,
        TestFileCheckpointStore::shared(companion(&path, ".external-witness")),
    )
    .unwrap();
    let authority = open_authority(&path);
    assert!(authority
        .ensure_ai_keeper_model(
            &revoked,
            revoked.model_id(),
            revoked.model_artifact_sha256()
        )
        .is_err());
    drop(authority);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn certification_ledger_child_process() {
    let Some(path) = std::env::var_os(CHILD_PATH) else {
        return;
    };
    let model_id = std::env::var(CHILD_ID).unwrap();
    issue(&open_authority(Path::new(&path)), &model_id);
}

#[test]
fn certification_registry_serializes_multiprocess_writers() {
    let root = test_root();
    let path = root.join("certification-registry.jsonl");
    drop(open_authority(&path));
    let executable = std::env::current_exe().unwrap();
    let mut children = Vec::new();
    for index in 0..8 {
        children.push(
            Command::new(&executable)
                .args([
                    "--exact",
                    "certification_ledger_child_process",
                    "--nocapture",
                ])
                .env(CHILD_PATH, &path)
                .env(CHILD_ID, format!("concurrent-model-{index}"))
                .spawn()
                .unwrap(),
        );
    }
    for child in children {
        assert!(child.wait_with_output().unwrap().status.success());
    }
    drop(open_authority(&path));
    let records: Vec<Value> = registry_lines(&path)
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 8);
    assert!(records
        .iter()
        .enumerate()
        .all(|(index, record)| record["sequence"] == index as u64 + 1));
    assert_eq!(
        records
            .iter()
            .map(|record| record["record_hash"].as_str().unwrap())
            .collect::<HashSet<_>>()
            .len(),
        8
    );
    fs::remove_dir_all(root).unwrap();
}
